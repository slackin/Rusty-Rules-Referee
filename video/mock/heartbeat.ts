/**
 * Keeps seeded "remote" servers (IDs 2 & 3) online by ticking their
 * `last_seen` column every 10 seconds. Server 1 is the master's own
 * self-registration and is refreshed by the live bot process.
 */
import Database from 'better-sqlite3';
import { paths } from '../lib/config.js';

const db = new Database(paths.db);
db.pragma('journal_mode = WAL');

const stmt = db.prepare(`UPDATE servers
  SET last_seen = CURRENT_TIMESTAMP,
      status = 'online',
      updated_at = CURRENT_TIMESTAMP
  WHERE id IN (2, 3)`);

function tick() {
  try {
    const r = stmt.run();
    if (process.env.HEARTBEAT_VERBOSE) {
      console.log(`[heartbeat] refreshed ${r.changes} server(s)`);
    }
  } catch (err: any) {
    console.error('[heartbeat]', err.message);
  }
}

tick();
const interval = setInterval(tick, 10_000);

process.on('SIGINT',  () => { clearInterval(interval); db.close(); process.exit(0); });
process.on('SIGTERM', () => { clearInterval(interval); db.close(); process.exit(0); });
console.log('[heartbeat] ticking every 10s for server IDs 2, 3');
