/**
 * Start the demo stack in **master mode** for the multi-server video.
 *
 *   1. Bootstrap mTLS certs (idempotent).
 *   2. Compute the client cert's SHA-256 fingerprint and patch the seeded
 *      server id=1 row's cert_fingerprint so that when the live client bot
 *      registers, the master updates server 1 in place (instead of creating
 *      a new id=4 row).
 *   3. Launch mock RCONs on UDP 127.0.0.1:27960 (master) and 27961 (client).
 *   4. Launch mock update-manifest server on http://127.0.0.1:8090.
 *   5. Launch heartbeat writer — keeps seeded servers 2 & 3 "online"
 *      (server 1 is kept alive by the live client's registration/status).
 *   6. Launch R3 binary in master mode, wait for /api/v1/setup/status → master.
 *   7. Launch R3 binary in client mode, wait for it to register and for
 *      server 1 to flip from SEED-SRV-* → real SHA-256 fingerprint.
 *   8. Write out/demo-ids.json with LIVE_SERVER_ID=1 for the recorder.
 *
 * Writes child PIDs to out/demo-stack.pid for stop-demo-stack.ts.
 */
import { spawn, ChildProcess, execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import Database from 'better-sqlite3';
import { fileURLToPath } from 'node:url';
import { paths, env } from '../lib/config.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

function log(msg: string) { console.log(`[stack] ${msg}`); }

function clientCertFingerprint(): string {
  const cliCrt = path.join(paths.out, 'certs', 'client.crt');
  if (!fs.existsSync(cliCrt)) throw new Error(`client cert missing at ${cliCrt}`);
  const x509 = new crypto.X509Certificate(fs.readFileSync(cliCrt));
  const der = x509.raw;
  const hex = crypto.createHash('sha256').update(der).digest('hex').toUpperCase();
  return hex.match(/../g)!.join(':');
}

function patchServerFingerprint(fp: string) {
  const db = new Database(paths.db);
  const info = db.prepare('UPDATE servers SET cert_fingerprint=? WHERE id=1').run(fp);
  db.close();
  log(`patched server id=1 cert_fingerprint → ${fp.slice(0, 23)}... (rows=${info.changes})`);
}

async function waitForUrl(url: string, timeoutMs: number): Promise<Response> {
  const start = Date.now();
  let lastErr: any = null;
  while (Date.now() - start < timeoutMs) {
    try {
      const r = await fetch(url, { signal: AbortSignal.timeout(2000) });
      if (r.ok || r.status === 401) return r;
    } catch (err) { lastErr = err; }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error(`timeout waiting for ${url} (${lastErr?.message ?? 'no response'})`);
}

async function waitForMasterMode(url: string, timeoutMs = 60_000) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const r = await fetch(`${url}/api/v1/setup/status`, { signal: AbortSignal.timeout(2000) });
      if (r.ok) {
        const body = await r.json() as { mode?: string };
        if (body.mode === 'master') return;
        log(`  setup/status mode=${body.mode}, still waiting...`);
      }
    } catch { /* retry */ }
    await new Promise((r) => setTimeout(r, 1000));
  }
  throw new Error('master mode never reached');
}

/** Wait until server id=1 row's cert_fingerprint is no longer the SEED-SRV-*
 * placeholder — which means the client has registered and the master has
 * rewritten the row. Uses the DB directly since the /api/v1/servers route
 * doesn't expose cert_fingerprint to clients. */
async function waitForClientRegistration(timeoutMs = 45_000) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const db = new Database(paths.db, { readonly: true, fileMustExist: true });
      const row = db.prepare('SELECT cert_fingerprint, status FROM servers WHERE id=1').get() as
        { cert_fingerprint?: string; status?: string } | undefined;
      db.close();
      if (row?.cert_fingerprint && !row.cert_fingerprint.startsWith('SEED-SRV-')) {
        log(`  client registered: server 1 status=${row.status}, fp=${row.cert_fingerprint.slice(0, 23)}...`);
        return;
      }
    } catch (err: any) {
      // DB may be locked by the master — retry
    }
    await new Promise((r) => setTimeout(r, 1000));
  }
  throw new Error('client never registered with master');
}

async function main() {
  if (!fs.existsSync(paths.db)) throw new Error('demo.db missing — run `npm run seed` first');
  if (!fs.existsSync(env.binary)) {
    throw new Error(
      `R3 binary not found at ${env.binary}\n` +
      `Build it first from the workspace root: cargo build --release`
    );
  }

  fs.mkdirSync(paths.out, { recursive: true });

  log('bootstrapping mTLS certs');
  execFileSync('tsx', [path.join(__dirname, '..', 'mock', 'bootstrap-certs.ts')], {
    stdio: 'inherit', shell: true,
  });

  const clientFp = clientCertFingerprint();
  patchServerFingerprint(clientFp);

  const masterConfig = path.join(paths.out, 'referee-demo-master.toml');
  const clientConfig = path.join(paths.out, 'referee-demo-client.toml');
  if (!fs.existsSync(masterConfig)) throw new Error(`master config missing — run npm run seed`);
  if (!fs.existsSync(clientConfig)) throw new Error(`client config missing — run npm run seed`);

  log('starting mock RCON (master) on 127.0.0.1:27960');
  const rconProc = spawn('tsx', [path.join(__dirname, '..', 'mock', 'rcon.ts')], {
    stdio: ['ignore', 'inherit', 'inherit'],
    shell: true,
    env: { ...process.env, MOCK_RCON_PORT: '27960' },
  });

  log('starting mock RCON (client) on 127.0.0.1:27961');
  const rconClientProc = spawn('tsx', [path.join(__dirname, '..', 'mock', 'rcon.ts')], {
    stdio: ['ignore', 'inherit', 'inherit'],
    shell: true,
    env: { ...process.env, MOCK_RCON_PORT: '27961' },
  });

  log('starting mock update server on 127.0.0.1:8090');
  const updateProc = spawn('tsx', [path.join(__dirname, '..', 'mock', 'update-server.ts')], {
    stdio: ['ignore', 'inherit', 'inherit'],
    shell: true,
  });

  log('starting heartbeat writer (servers 2, 3)');
  const heartbeatProc = spawn('tsx', [path.join(__dirname, '..', 'mock', 'heartbeat.ts')], {
    stdio: ['ignore', 'inherit', 'inherit'],
    shell: true,
  });

  await new Promise((r) => setTimeout(r, 1000));

  log(`starting R3 (master mode): ${env.binary} ${masterConfig}`);
  const botProc = spawn(env.binary, [masterConfig, '--mode', 'master'], {
    stdio: ['ignore', 'inherit', 'inherit'],
    env: { ...process.env, RUST_LOG: 'info' },
  });

  const procs: Record<string, ChildProcess> = {
    rcon: rconProc, rconClient: rconClientProc, update: updateProc,
    heartbeat: heartbeatProc, bot: botProc,
  };
  const pids: Record<string, number | undefined> = {
    rcon: rconProc.pid, rconClient: rconClientProc.pid, update: updateProc.pid,
    heartbeat: heartbeatProc.pid, bot: botProc.pid,
  };

  log(`waiting for ${env.url}/api/v1/setup/status (mode=master) ...`);
  try {
    await waitForUrl(`${env.url}/api/v1/setup/status`, 60_000);
    await waitForMasterMode(env.url, 60_000);
  } catch (err: any) {
    log(`master stack did not come up: ${err.message}`);
    fs.writeFileSync(paths.pidFile, JSON.stringify(pids, null, 2));
    killAll(procs);
    process.exit(1);
  }
  log('master is up.');

  log(`starting R3 (client mode): ${env.binary} ${clientConfig}`);
  const clientProc = spawn(env.binary, [clientConfig, '--mode', 'client'], {
    stdio: ['ignore', 'inherit', 'inherit'],
    env: { ...process.env, RUST_LOG: 'info' },
  });
  procs.client = clientProc;
  pids.client = clientProc.pid;
  fs.writeFileSync(paths.pidFile, JSON.stringify(pids, null, 2));
  log(`pids: ${JSON.stringify(pids)}`);

  try {
    await waitForClientRegistration(60_000);
  } catch (err: any) {
    log(`client did not register: ${err.message}`);
    killAll(procs);
    process.exit(1);
  }

  // Record live server id for the recorder's URL substitution.
  fs.writeFileSync(
    path.join(paths.out, 'demo-ids.json'),
    JSON.stringify({ LIVE_SERVER_ID: 1 }, null, 2),
  );
  log('wrote demo-ids.json (LIVE_SERVER_ID=1)');

  log('stack is fully up (master + client + mocks).');

  process.on('SIGINT',  () => killAll(procs));
  process.on('SIGTERM', () => killAll(procs));
  setInterval(() => {}, 1 << 30);
}

function killAll(procs: Record<string, ChildProcess>) {
  for (const [name, p] of Object.entries(procs)) {
    try { p.kill('SIGTERM'); } catch { /* already dead */ }
    log(`  killed ${name}`);
  }
  if (fs.existsSync(paths.pidFile)) fs.unlinkSync(paths.pidFile);
  process.exit(0);
}

main().catch((err) => { console.error(err); process.exit(1); });
