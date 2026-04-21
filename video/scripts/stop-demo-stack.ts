/**
 * Terminate processes recorded in out/demo-stack.pid.
 */
import fs from 'node:fs';
import { paths } from '../lib/config.js';

if (!fs.existsSync(paths.pidFile)) {
  console.log('[stack] no pid file; nothing to stop');
  process.exit(0);
}
const pids = JSON.parse(fs.readFileSync(paths.pidFile, 'utf8')) as { rcon?: number; bot?: number };
for (const [name, pid] of Object.entries(pids)) {
  if (!pid) continue;
  try {
    process.kill(pid, 'SIGTERM');
    console.log(`[stack] stopped ${name} pid=${pid}`);
  } catch (err: any) {
    console.log(`[stack] could not stop ${name} pid=${pid}: ${err.message}`);
  }
}
fs.unlinkSync(paths.pidFile);
