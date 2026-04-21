/**
 * Mock update manifest server.
 *
 * Serves a fake `latest.json` on 127.0.0.1:8090 that advertises an upgrade
 * from the bot's current version to "2.1.0-demo". The UI's "Check for
 * updates" button will then reveal the amber "update available" card.
 *
 * We never actually serve the binary itself; the download URL points back
 * at this server at /downloads/... and we return a 404 so the Apply button
 * surfaces a graceful error if clicked. For the demo, we stop at the
 * "available" state, which is the interesting part visually.
 */
import http from 'node:http';

const PORT = parseInt(process.env.UPDATE_PORT ?? '8090', 10);

const MANIFEST = {
  channel: 'beta',
  version: '2.1.0-demo',
  build_hash: '2.1.0-demo-FEEDFACE',
  git_commit: 'feedfacedeadbeef0123456789abcdef01234567',
  release_notes_url: 'https://r3.pugbot.net/changelog',
  notes: 'Multi-server polish, improved stats, security fixes',
  released_at: new Date().toISOString(),
  platforms: {
    'windows-x86_64': {
      url:    `http://127.0.0.1:${PORT}/downloads/rusty-rules-referee-2.1.0-demo-windows.exe`,
      sha256: '0000000000000000000000000000000000000000000000000000000000000000',
      size: 18234567,
    },
    'linux-x86_64': {
      url:    `http://127.0.0.1:${PORT}/downloads/rusty-rules-referee-2.1.0-demo-linux`,
      sha256: '1111111111111111111111111111111111111111111111111111111111111111',
      size: 18987654,
    },
  },
};

const server = http.createServer((req, res) => {
  const rawUrl = req.url || '/';
  const url = rawUrl.split('?', 1)[0]!;
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Headers', '*');
  console.log(`[update-server] ${req.method} ${rawUrl}`);

  // Channel manifests: /beta/latest.json, /alpha/latest.json, /stable/latest.json
  if (/^\/(beta|alpha|stable)\/latest\.json$/.test(url)) {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify(MANIFEST, null, 2));
    return;
  }

  // Legacy/flat path
  if (url === '/latest.json') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify(MANIFEST, null, 2));
    return;
  }

  res.writeHead(404, { 'Content-Type': 'text/plain' });
  res.end('not found\n');
});

server.listen(PORT, '127.0.0.1', () => {
  console.log(`[update-server] listening on http://127.0.0.1:${PORT}`);
  console.log(`[update-server]   GET /beta/latest.json  -> v${MANIFEST.version}`);
});

process.on('SIGINT',  () => { server.close(); process.exit(0); });
process.on('SIGTERM', () => { server.close(); process.exit(0); });
