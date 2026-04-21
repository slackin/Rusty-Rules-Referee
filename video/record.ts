/**
 * Playwright walkthrough recorder.
 *
 * - Launches Chromium at 2560×1440 with recordVideo enabled.
 * - Injects a synthetic cursor (Playwright recordings don't render the OS cursor).
 * - Executes each scene from script/narration.json sequentially.
 * - Produces a single WebM in out/raw/<timestamp>.webm which build-video.ts
 *   transcodes to the final MP4 masters.
 */
import { chromium, Page } from 'playwright';
import fs from 'node:fs';
import path from 'node:path';
import { paths, env, Script, Scene, Interaction } from './lib/config.js';

const VIEWPORT = { width: 2560, height: 1440 };

function log(msg: string) { console.log(`[record] ${msg}`); }

const CURSOR_JS = `
(() => {
  if (window.__r3Cursor) return;
  const c = document.createElement('div');
  c.id = '__r3-cursor';
  c.style.cssText = [
    'position:fixed','top:0','left:0','width:32px','height:32px',
    'pointer-events:none','z-index:2147483647','transition:transform 60ms linear',
    'background-image:url("data:image/svg+xml;utf8,' +
      encodeURIComponent(
        '<svg xmlns=\\'http://www.w3.org/2000/svg\\' width=\\'32\\' height=\\'32\\' viewBox=\\'0 0 32 32\\'>' +
        '<path d=\\'M4 2 L4 26 L10 20 L14 28 L18 26 L14 18 L22 18 Z\\' fill=\\'white\\' stroke=\\'black\\' stroke-width=\\'1.5\\' stroke-linejoin=\\'round\\'/>' +
        '</svg>'
      ) + '")',
    'background-repeat:no-repeat','background-size:contain',
    'filter:drop-shadow(0 2px 4px rgba(0,0,0,0.5))'
  ].join(';');
  document.documentElement.appendChild(c);
  window.addEventListener('mousemove', (e) => {
    c.style.transform = 'translate(' + e.clientX + 'px,' + e.clientY + 'px)';
  }, { passive: true });
  window.__r3Cursor = c;
})();
`;

async function mouseGlide(page: Page, toX: number, toY: number, steps = 24) {
  // Approximate current position from last known; Playwright's mouse has no getter,
  // so we just steer smoothly toward the target in fixed steps.
  const box = await page.viewportSize();
  if (!box) return;
  const startX = box.width / 2;
  const startY = box.height / 2;
  for (let i = 1; i <= steps; i++) {
    const t = i / steps;
    // ease-in-out
    const e = t < 0.5 ? 2 * t * t : -1 + (4 - 2 * t) * t;
    const x = startX + (toX - startX) * e;
    const y = startY + (toY - startY) * e;
    await page.mouse.move(x, y);
    await page.waitForTimeout(20);
  }
}

async function runInteraction(page: Page, i: Interaction) {
  switch (i.type) {
    case 'waitForSelector':
      if (i.selector) {
        try { await page.waitForSelector(i.selector, { timeout: i.timeoutMs ?? 5000 }); }
        catch { /* selector optional; continue */ }
      }
      break;
    case 'idle':
      await page.waitForTimeout(i.ms ?? 1000);
      break;
    case 'glide': {
      const el = i.selector ? await page.$(i.selector) : null;
      const box = el ? await el.boundingBox() : null;
      const vp = page.viewportSize()!;
      const tx = box ? box.x + box.width / 2 : vp.width / 2;
      const ty = box ? box.y + box.height / 2 : vp.height / 2;
      await mouseGlide(page, tx, ty);
      break;
    }
    case 'hover':
      if (i.selector) {
        try { await page.hover(i.selector, { timeout: 2000 }); } catch {}
      }
      break;
    case 'type':
      if (i.selector && i.text !== undefined) {
        const el = await page.$(i.selector);
        if (el) {
          await el.click();
          await page.keyboard.type(i.text, { delay: i.delay ?? 80 });
        } else if (!i.ifExists) {
          log(`  type: selector not found: ${i.selector}`);
        }
      }
      break;
    case 'clearInput':
      if (i.selector) {
        const el = await page.$(i.selector);
        if (el) {
          await el.click();
          await page.keyboard.press('Control+A');
          await page.keyboard.press('Delete');
        }
      }
      break;
    case 'press':
      if (i.key) await page.keyboard.press(i.key);
      break;
    case 'click':
      if (i.selector) {
        try {
          await page.click(i.selector, { timeout: 3000 });
        } catch {
          if (!i.ifExists) log(`  click: ${i.selector} not clickable`);
        }
      }
      break;
    case 'select':
      if (i.selector && i.value !== undefined) {
        try { await page.selectOption(i.selector, i.value, { timeout: 3000 }); }
        catch {
          if (!i.ifExists) log(`  select: ${i.selector} not found`);
        }
      }
      break;
    case 'scroll':
      await page.mouse.wheel(0, i.y ?? 400);
      break;
  }
}

async function login(page: Page) {
  await page.goto(`${env.url}/login`, { waitUntil: 'domcontentloaded' });
  await page.waitForSelector('input[type="text"], input[autocomplete="username"]', { timeout: 10000 });
  await page.fill('input[autocomplete="username"], input[type="text"]', env.adminUser);
  await page.fill('input[type="password"]', env.adminPass);
  await Promise.all([
    page.waitForURL((url) => !url.pathname.startsWith('/login'), { timeout: 15000 }),
    page.click('button[type="submit"]'),
  ]);
  log('logged in');
}

async function playScene(page: Page, scene: Scene) {
  log(`scene: ${scene.id} → ${scene.route} (${scene.durationMs}ms)`);
  const started = Date.now();
  try {
    await page.goto(`${env.url}${scene.route}`, { waitUntil: 'networkidle', timeout: 15000 });
  } catch {
    log(`  networkidle timeout on ${scene.route} — continuing`);
  }
  // Re-inject cursor in case navigation cleared it
  await page.evaluate(CURSOR_JS).catch(() => {});
  for (const inter of scene.interactions) {
    if (inter.atMs !== undefined) {
      const targetT = started + inter.atMs;
      const waitMs = targetT - Date.now();
      if (waitMs > 0) await page.waitForTimeout(waitMs);
    }
    await runInteraction(page, inter);
  }
  // Pad to the scene's total duration (accounts for interactions consuming time)
  const elapsed = Date.now() - started;
  const remaining = scene.durationMs - elapsed;
  if (remaining > 0) await page.waitForTimeout(remaining);
}

function substituteUrlPlaceholders(script: Script): Script {
  const idsPath = path.join(paths.out, 'demo-ids.json');
  if (!fs.existsSync(idsPath)) return script;
  const ids = JSON.parse(fs.readFileSync(idsPath, 'utf8')) as Record<string, number | string>;
  const tokens: Record<string, string> = {};
  for (const [k, v] of Object.entries(ids)) tokens[k] = String(v);
  const replaceStr = (s: string) => s.replace(/\{([A-Z_][A-Z0-9_]*)\}/g, (m, name) => tokens[name] ?? m);
  return {
    ...script,
    scenes: script.scenes.map((sc) => ({
      ...sc,
      route: replaceStr(sc.route),
      interactions: sc.interactions.map((i) => ({
        ...i,
        selector: i.selector ? replaceStr(i.selector) : i.selector,
        text: i.text !== undefined ? replaceStr(i.text) : i.text,
      })),
    })),
  };
}

async function main() {
  // Prefer the timed script (post-narration) so each scene's recorded
  // duration matches the padded audio length, keeping A/V in sync.
  const scriptPath = fs.existsSync(paths.timed) ? paths.timed : paths.script;
  log(`using script: ${scriptPath}`);
  const rawScript: Script = JSON.parse(fs.readFileSync(scriptPath, 'utf8'));
  const script = substituteUrlPlaceholders(rawScript);
  fs.mkdirSync(paths.raw, { recursive: true });

  const browser = await chromium.launch({
    args: [
      '--force-device-scale-factor=1',
      '--disable-background-timer-throttling',
      '--disable-renderer-backgrounding',
    ],
  });
  const context = await browser.newContext({
    viewport: VIEWPORT,
    deviceScaleFactor: 1,
    recordVideo: { dir: paths.raw, size: VIEWPORT },
    reducedMotion: 'no-preference',
    ignoreHTTPSErrors: true,
  });
  // Inject cursor into every page, including before navigation.
  await context.addInitScript(CURSOR_JS);

  const page = await context.newPage();

  try {
    await login(page);

    for (const scene of script.scenes) {
      await playScene(page, scene);
    }

    log('walkthrough complete');
  } finally {
    const video = page.video();
    await context.close();
    await browser.close();
    if (video) {
      const rawPath = await video.path();
      const finalRaw = path.join(paths.raw, 'walkthrough.webm');
      fs.renameSync(rawPath, finalRaw);
      log(`raw video: ${finalRaw}`);
    }
  }
}

main().catch((err) => { console.error(err); process.exit(1); });
