/**
 * Upload rendered MP4s + poster to r3.pugbot.net via rsync (when available)
 * or scp (fallback, e.g. on Windows with OpenSSH).
 * Requires ssh key auth to be already configured (same key used by deploy.ps1).
 */
import fs from 'node:fs';
import { spawnSync } from 'node:child_process';
import { paths, env } from '../lib/config.js';

function log(msg: string) { console.log(`[publish] ${msg}`); }

function hasCommand(cmd: string): boolean {
  const probe = process.platform === 'win32'
    ? spawnSync('where', [cmd], { stdio: 'ignore' })
    : spawnSync('which', [cmd], { stdio: 'ignore' });
  return probe.status === 0;
}

function main() {
  const files = ['r3-demo-1440p.mp4', 'r3-demo-1080p.mp4', 'r3-demo-720p.mp4', 'r3-demo-poster.jpg'];
  for (const f of files) {
    const p = `${paths.final}/${f}`;
    if (!fs.existsSync(p)) throw new Error(`missing ${p} — run \`npm run build\` first`);
  }

  const sshTarget = `${env.publishSshUser}@${env.publishSshHost}`;

  // Ensure remote directory exists
  log(`ensuring ${env.publishPath} exists on ${sshTarget}`);
  let r = spawnSync('ssh', [sshTarget, `mkdir -p ${env.publishPath}`], { stdio: 'inherit' });
  if (r.status !== 0) throw new Error('ssh mkdir failed');

  const target = `${sshTarget}:${env.publishPath}/`;
  if (hasCommand('rsync')) {
    log(`rsyncing to ${target}`);
    r = spawnSync(
      'rsync',
      ['-avz', '--progress', ...files.map((f) => `${paths.final}/${f}`), target],
      { stdio: 'inherit' }
    );
    if (r.status !== 0) throw new Error('rsync failed');
  } else {
    log(`rsync not found — using scp to upload to ${target}`);
    for (const f of files) {
      const src = `${paths.final}/${f}`;
      log(`  scp ${f}`);
      r = spawnSync('scp', ['-q', src, target], { stdio: 'inherit' });
      if (r.status !== 0) throw new Error(`scp failed for ${f}`);
    }
  }

  // Fix ownership so the webserver (running as bcmx) can serve these files.
  log(`chown -R ${env.publishOwner} ${env.publishPath}`);
  r = spawnSync('ssh', [sshTarget, `chown -R ${env.publishOwner} ${env.publishPath}`], { stdio: 'inherit' });
  if (r.status !== 0) throw new Error('ssh chown failed');

  log('published. URLs:');
  for (const f of files) log(`  https://${env.publishHost}/media/${f}`);
}

main();
