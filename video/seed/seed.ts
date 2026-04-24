/**
 * Seed a fresh SQLite database with realistic multi-server demo data
 * for the R3 video walkthrough.
 *
 * Layout:
 *   - 3 fake servers (1 = "real" client-paired, 2 & 3 = seed-only peers).
 *   - 60 global clients split 20/20/20 across the three servers for XLR stats
 *     (xlr_playerstats has UNIQUE(client_id) so each client belongs to exactly
 *     one server). Clients themselves are global, so aliases, penalties, chat,
 *     and audit_log can reference any client_id from any server.
 *   - 30 days of xlr_history rows per top-player so the dashboard trend chart
 *     looks alive.
 *   - Fresh `last_seen` on all 3 servers so `is_server_online()` treats them
 *     as online at boot. The keep-alive heartbeat writer keeps it that way.
 */
import Database from 'better-sqlite3';
import bcrypt from 'bcryptjs';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { paths, env } from '../lib/config.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const migrationsDir = path.resolve(__dirname, '..', '..', 'migrations');

function log(msg: string) { console.log(`[seed] ${msg}`); }

// -------------------------------------------------------------------------
// Schema helpers
// -------------------------------------------------------------------------

function applyMigrations(db: Database.Database) {
  const files = fs.readdirSync(migrationsDir).filter((f) => f.endsWith('.sql')).sort();
  for (const f of files) {
    const sql = fs.readFileSync(path.join(migrationsDir, f), 'utf8');
    try {
      db.exec(sql);
      log(`applied ${f}`);
    } catch (err: any) {
      log(`warning in ${f}: ${err.message}`);
    }
  }
}

function tableExists(db: Database.Database, name: string): boolean {
  return !!db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name=?").get(name);
}

function columns(db: Database.Database, table: string): Set<string> {
  const rows = db.prepare(`PRAGMA table_info(${table})`).all() as Array<{ name: string }>;
  return new Set(rows.map((r) => r.name));
}

// -------------------------------------------------------------------------
// Fixture data
// -------------------------------------------------------------------------

interface ServerDef {
  id: number;
  name: string;
  address: string;
  port: number;
  maxClients: number;
  currentMap: string;
  playerCount: number;
  channel: string;
  fingerprint: string;
}

const SERVERS: ServerDef[] = [
  { id: 1, name: 'R3 US East - Turnpike', address: '127.0.0.1',    port: 27961,
    maxClients: 16, currentMap: 'ut4_turnpike', playerCount: 11, channel: 'beta',
    fingerprint: 'SEED-SRV-00001' },
  { id: 2, name: 'R3 EU West - Classic',  address: '203.0.113.14', port: 27960,
    maxClients: 20, currentMap: 'ut4_casa',     playerCount: 8,  channel: 'beta',
    fingerprint: 'SEED-SRV-00002' },
  { id: 3, name: 'R3 OCE Sydney - 24/7',  address: '198.51.100.7', port: 27960,
    maxClients: 32, currentMap: 'ut4_abbey',    playerCount: 5,  channel: 'alpha',
    fingerprint: 'SEED-SRV-00003' },
];

const FIRST_NAMES = [
  'Sn1per','GhostRider','FragLord','NoScope','Vortex','Banshee','Reaper','Kingpin',
  'Mystique','Onyx','PixelPusher','Quickshot','Raven','Shadow','Tango','Umbra',
  'Viper','Wraith','Xero','Yoda','Zenith','Blaze','Cinder','Dagger','Eclipse',
  'Falcon','Grizzly','Havoc','Inferno','Jester','Kodiak','Lynx','Maverick','Nomad',
  'Oracle','Phantom','Quasar','Rogue','Sage','Titan','Undertaker','Vanguard','Wolf',
  'Xen','Yield','Zulu','Apex','Bullet','Crow','Drift','Echo','Fury','Ghost',
  'Hunter','Ion','Jackal','Krait','Lotus','Magma','Nightshade',
];

const MAPS_PER_SERVER: string[][] = [
  ['ut4_turnpike', 'ut4_uptown', 'ut4_kingdom'],
  ['ut4_casa', 'ut4_algiers', 'ut4_mandolin'],
  ['ut4_abbey', 'ut4_prague', 'ut4_ramelle'],
];

const WEAPONS = ['ut_weap_ak103', 'ut_weap_lr300', 'ut_weap_sr8', 'ut_weap_g36', 'ut_weap_negev', 'ut_weap_spas', 'ut_weap_hk69'];

const CHAT_LINES = [
  'gg all', 'nice shot', 'ff!', 'lagging hard', 'switch me',
  'nade incoming', 'push A', 'push B', 'rotate', 'rush red',
  'sniper on cat', 'anyone spec?', 'rematch?', 'gl hf', 'good game',
  'that was sick', 'bomb planted', 'flag taken', 'need backup mid',
  'camping at spawn', 'rcon please', 'bot down', 'rush now',
  'team spread', 'top rotation', 'bottom clear', 'my bad',
  'one left', 'push together', 'all rotate', 'def B', 'def A',
  'top control', 'nade out', 'flash in', 'push site',
];

// Deterministic PRNG so renders are reproducible
let rngSeed = 0x12345678;
function rand() {
  rngSeed = (rngSeed * 1103515245 + 12345) & 0x7fffffff;
  return rngSeed / 0x7fffffff;
}
function randInt(min: number, max: number) { return Math.floor(rand() * (max - min + 1)) + min; }
function pick<T>(arr: T[]): T { return arr[Math.floor(rand() * arr.length)]; }

// -------------------------------------------------------------------------
// Seed functions
// -------------------------------------------------------------------------

function seedServers(db: Database.Database) {
  if (!tableExists(db, 'servers')) {
    log('no servers table - skipping (pre-006 schema)');
    return;
  }
  const cols = columns(db, 'servers');
  const hasChannel = cols.has('update_channel');
  const fields = ['id','name','address','port','status','current_map','player_count','max_clients','cert_fingerprint','config_version'];
  if (hasChannel) fields.push('update_channel');
  fields.push('last_seen', 'created_at', 'updated_at');
  const placeholders = fields.map((f) =>
    (f === 'last_seen' || f === 'created_at' || f === 'updated_at') ? 'CURRENT_TIMESTAMP' : '?'
  ).join(', ');
  const stmt = db.prepare(`INSERT INTO servers (${fields.join(', ')}) VALUES (${placeholders})`);

  for (const srv of SERVERS) {
    const vals: Record<string, any> = {
      id: srv.id, name: srv.name, address: srv.address, port: srv.port,
      status: 'online', current_map: srv.currentMap,
      player_count: srv.playerCount, max_clients: srv.maxClients,
      cert_fingerprint: srv.fingerprint, config_version: 1,
      update_channel: srv.channel,
    };
    const args = fields
      .filter((f) => f !== 'last_seen' && f !== 'created_at' && f !== 'updated_at')
      .map((f) => vals[f]);
    try { stmt.run(...args); }
    catch (err: any) { log(`server row ${srv.id} failed: ${err.message}`); }
  }
  log(`seeded ${SERVERS.length} servers`);
}

function seedClients(db: Database.Database) {
  if (!tableExists(db, 'clients')) return;
  const cols = columns(db, 'clients');
  if (!cols.has('guid')) return;
  const hasAuth = cols.has('auth_name');
  const fields = ['guid','pbid','name','ip','greeting','login','group_bits','auto_login','last_visit'];
  if (hasAuth) fields.splice(fields.indexOf('login') + 1, 0, 'auth_name');
  const placeholders = fields.map((f) => f === 'last_visit' ? "datetime('now', ?)" : '?').join(', ');
  const stmt = db.prepare(`INSERT INTO clients (${fields.join(', ')}) VALUES (${placeholders})`);

  FIRST_NAMES.forEach((name, i) => {
    const lastVisit = `-${randInt(0, 30)} days`;
    const group = i === 0 ? 128 : i === 1 ? 64 : i === 2 ? 16 : i < 9 ? 2 : 1;
    const values: Record<string, any> = {
      guid: `GUID${String(i + 1).padStart(8, '0')}`,
      pbid: '', name,
      ip: `10.${randInt(0,255)}.${randInt(0,255)}.${randInt(0,255)}`,
      greeting: i === 0 ? '^3welcome back, boss' : '',
      login: i < 3 ? name.toLowerCase() : '',
      auth_name: i < 3 ? name.toLowerCase() : '',
      group_bits: group, auto_login: 1, last_visit: lastVisit,
    };
    try { stmt.run(...fields.map((f) => values[f])); }
    catch (err: any) { log(`clients row ${i} failed: ${err.message}`); }
  });
  log(`seeded ${FIRST_NAMES.length} clients`);
}

function seedAliases(db: Database.Database) {
  if (!tableExists(db, 'aliases')) return;
  const stmt = db.prepare(`INSERT INTO aliases (client_id, alias, num_used) VALUES (?, ?, ?)`);
  for (let id = 1; id <= FIRST_NAMES.length; id++) {
    const count = 1 + randInt(0, 2);
    for (let a = 0; a < count; a++) {
      stmt.run(id, `${FIRST_NAMES[id - 1]}_${a + 1}`, randInt(1, 50));
    }
  }
  log('seeded aliases');
}

function buildClientServerMap(): Map<number, number> {
  const m = new Map<number, number>();
  const perServer = Math.floor(FIRST_NAMES.length / SERVERS.length);
  for (let i = 0; i < FIRST_NAMES.length; i++) {
    const srvIdx = Math.min(Math.floor(i / perServer), SERVERS.length - 1);
    m.set(i + 1, SERVERS[srvIdx].id);
  }
  return m;
}

function seedPenalties(db: Database.Database, clientServer: Map<number, number>) {
  if (!tableExists(db, 'penalties')) return;
  const cols = columns(db, 'penalties');
  const hasServerId = cols.has('server_id');
  const hasScope = cols.has('scope');
  const fields = ['type','client_id','admin_id','duration','reason','keyword','inactive'];
  if (hasServerId) fields.push('server_id');
  if (hasScope) fields.push('scope');
  fields.push('time_add');
  const placeholders = fields.map((f) => f === 'time_add' ? "datetime('now', ?)" : '?').join(', ');
  const stmt = db.prepare(`INSERT INTO penalties (${fields.join(', ')}) VALUES (${placeholders})`);

  const reasons: Array<[string, number | null, string, string]> = [
    ['Ban',     null, 'cheating - aimbot detected',    'cheat'],
    ['Ban',     null, 'wallhack - 92% headshot ratio', 'cheat'],
    ['TempBan', 1440, 'racism in global chat',         'racism'],
    ['TempBan', 720,  'spawn kill abuse',              'spawnkill'],
    ['TempBan', 60,   'team killing - repeated',       'tk'],
    ['TempBan', 30,   'vote abuse',                    'vote'],
    ['Warn',    null, 'spam in global chat',           'spam'],
    ['Warn',    null, 'bad language',                  'lang'],
    ['Warn',    null, 'ping too high',                 'ping'],
    ['Warn',    null, 'AFK in spec',                   'afk'],
    ['Kick',    null, 'high ping (>350ms)',            'ping'],
    ['Kick',    null, 'forbidden name',                'name'],
    ['TempBan', 10,   'rage quit abuse',               'rage'],
  ];

  let total = 0;
  for (const srv of SERVERS) {
    const count = randInt(8, 12);
    for (let i = 0; i < count; i++) {
      const [type, duration, reason, keyword] = reasons[randInt(0, reasons.length - 1)];
      const eligible = [...clientServer.entries()].filter(([, s]) => s === srv.id).map(([c]) => c);
      const cid = pick(eligible);
      const vals: Record<string, any> = {
        type, client_id: cid, admin_id: randInt(1, 3), duration,
        reason, keyword, inactive: rand() < 0.15 ? 1 : 0,
        server_id: srv.id, scope: 'local',
        time_add: `-${randInt(1, 72)} hours`,
      };
      try { stmt.run(...fields.map((f) => vals[f])); total++; } catch {}
    }
  }
  log(`seeded ${total} penalties across ${SERVERS.length} servers`);
}

function seedChat(db: Database.Database, clientServer: Map<number, number>) {
  const candidates = ['chat_messages', 'chatlog', 'chat_log', 'chatlogs'];
  const t = candidates.find((c) => tableExists(db, c));
  if (!t) return;
  const cols = columns(db, t);
  const msgCol = cols.has('message') ? 'message' : cols.has('msg') ? 'msg' : null;
  const clientCol = cols.has('client_id') ? 'client_id' : cols.has('cid') ? 'cid' : null;
  const timeCol = cols.has('time_add') ? 'time_add' : cols.has('created_at') ? 'created_at' : cols.has('timestamp') ? 'timestamp' : null;
  if (!msgCol || !clientCol || !timeCol) return;
  const hasName = cols.has('client_name');
  const hasChannel = cols.has('channel');
  const hasServerId = cols.has('server_id');

  const fields = [clientCol];
  if (hasName) fields.push('client_name');
  if (hasChannel) fields.push('channel');
  fields.push(msgCol);
  if (hasServerId) fields.push('server_id');
  fields.push(timeCol);
  const placeholders = fields.map((f) => f === timeCol ? `datetime('now', ?)` : '?').join(', ');
  const stmt = db.prepare(`INSERT INTO ${t} (${fields.join(', ')}) VALUES (${placeholders})`);

  let total = 0;
  for (const srv of SERVERS) {
    const count = randInt(70, 100);
    const eligible = [...clientServer.entries()].filter(([, s]) => s === srv.id).map(([c]) => c);
    for (let i = 0; i < count; i++) {
      const cid = pick(eligible);
      const vals: Record<string, any> = {
        [clientCol]: cid,
        client_name: FIRST_NAMES[cid - 1],
        channel: rand() < 0.2 ? 'team' : 'global',
        [msgCol]: pick(CHAT_LINES),
        server_id: srv.id,
        [timeCol]: `-${randInt(1, 180)} minutes`,
      };
      try { stmt.run(...fields.map((f) => vals[f])); total++; } catch {}
    }
  }
  log(`seeded ${total} chat lines into ${t}`);
}

function seedVoteHistory(db: Database.Database) {
  if (!tableExists(db, 'vote_history')) return;
  const cols = columns(db, 'vote_history');
  const fields = ['client_id','client_name','vote_type','vote_arg','passed','yes_votes','no_votes','time_add']
    .filter((f) => cols.has(f));
  if (!fields.length) return;
  const placeholders = fields.map((f) => f === 'time_add' ? "datetime('now', ?)" : '?').join(', ');
  const stmt = db.prepare(`INSERT INTO vote_history (${fields.join(', ')}) VALUES (${placeholders})`);

  const voteTypes: Array<[string, string]> = [
    ['map', 'ut4_turnpike'], ['map', 'ut4_casa'], ['map', 'ut4_abbey'],
    ['kick', 'Xero'], ['kick', 'Viper'],
    ['nextmap', 'ut4_uptown'], ['cyclemap', ''],
    ['g_gametype', '4'], ['timelimit', '15'],
  ];
  let total = 0;
  for (let s = 0; s < SERVERS.length; s++) {
    for (let i = 0; i < 10; i++) {
      const cid = randInt(1, FIRST_NAMES.length);
      const [t, arg] = voteTypes[randInt(0, voteTypes.length - 1)];
      const vals: Record<string, any> = {
        client_id: cid, client_name: FIRST_NAMES[cid - 1],
        vote_type: t, vote_arg: arg,
        passed: rand() < 0.6 ? 1 : 0,
        yes_votes: randInt(3, 12), no_votes: randInt(0, 6),
        time_add: `-${randInt(1, 720)} minutes`,
      };
      try { stmt.run(...fields.map((f) => vals[f])); total++; } catch {}
    }
  }
  log(`seeded ${total} vote_history rows`);
}

function seedXlrstats(db: Database.Database, clientServer: Map<number, number>) {
  if (!tableExists(db, 'xlr_playerstats')) return;
  const cols = columns(db, 'xlr_playerstats');
  const hasServerId = cols.has('server_id');
  const base = [
    'client_id','kills','deaths','teamkills','teamdeaths',
    'suicides','ratio','skill','assists','rounds',
    'winstreak','losestreak','curstreak','biggeststreak',
  ].filter((c) => cols.has(c));
  if (hasServerId) base.push('server_id');
  const stmt = db.prepare(
    `INSERT INTO xlr_playerstats (${base.join(', ')}) VALUES (${base.map(() => '?').join(', ')})`
  );
  let total = 0;
  for (let i = 0; i < FIRST_NAMES.length; i++) {
    const cid = i + 1;
    const srvId = clientServer.get(cid)!;
    const localRank = i % 20;
    const kills = 1800 - localRank * 60 + randInt(-40, 80);
    const deaths = 900 + localRank * 30 + randInt(-20, 40);
    const ratio = +(kills / Math.max(deaths, 1)).toFixed(2);
    const skill = 1800 - localRank * 28 + randInt(-50, 80);
    const vals: Record<string, any> = {
      client_id: cid, kills, deaths,
      teamkills: randInt(0, 12), teamdeaths: randInt(0, 12),
      suicides: randInt(0, 6), ratio, skill,
      assists: randInt(50, 400), rounds: 40 + localRank * 3,
      winstreak: randInt(0, 10), losestreak: randInt(0, 5),
      curstreak: randInt(-3, 8), biggeststreak: randInt(12, 40),
      server_id: srvId,
    };
    try { stmt.run(...base.map((c) => vals[c])); total++; }
    catch (err: any) { log(`xlr_playerstats row ${i} failed: ${err.message}`); }
  }
  log(`seeded ${total} xlr_playerstats rows`);

  if (tableExists(db, 'xlr_weaponstats')) {
    const wCols = columns(db, 'xlr_weaponstats');
    const list = ['client_id','name','kills','deaths','headshots','teamkills','suicides']
      .filter((c) => wCols.has(c));
    if (wCols.has('server_id')) list.push('server_id');
    const wstmt = db.prepare(
      `INSERT OR IGNORE INTO xlr_weaponstats (${list.join(', ')}) VALUES (${list.map(() => '?').join(', ')})`
    );
    let wt = 0;
    for (let p = 1; p <= FIRST_NAMES.length; p++) {
      const srvId = clientServer.get(p)!;
      for (const w of WEAPONS) {
        const v: Record<string, any> = {
          client_id: p, name: w,
          kills: randInt(30, 300), deaths: randInt(15, 180),
          headshots: randInt(5, 60),
          teamkills: randInt(0, 3), suicides: randInt(0, 2),
          server_id: srvId,
        };
        try { wstmt.run(...list.map((c) => v[c])); wt++; } catch {}
      }
    }
    log(`seeded ${wt} xlr_weaponstats rows`);
  }

  if (tableExists(db, 'xlr_weaponusage')) {
    const u = columns(db, 'xlr_weaponusage');
    const list = ['name','kills','deaths','headshots','teamkills','suicides'].filter((c) => u.has(c));
    const stmt = db.prepare(
      `INSERT OR IGNORE INTO xlr_weaponusage (${list.join(', ')}) VALUES (${list.map(() => '?').join(', ')})`
    );
    for (const w of WEAPONS) {
      const v: Record<string, any> = {
        name: w, kills: randInt(2000, 8000), deaths: randInt(1500, 6000),
        headshots: randInt(400, 2000), teamkills: randInt(10, 60), suicides: randInt(5, 40),
      };
      try { stmt.run(...list.map((c) => v[c])); } catch {}
    }
    log('seeded xlr_weaponusage');
  }

  if (tableExists(db, 'xlr_mapstats')) {
    const m = columns(db, 'xlr_mapstats');
    const list = ['name','rounds','kills','suicides','teamkills'].filter((c) => m.has(c));
    if (m.has('server_id')) list.push('server_id');
    const stmt = db.prepare(
      `INSERT OR IGNORE INTO xlr_mapstats (${list.join(', ')}) VALUES (${list.map(() => '?').join(', ')})`
    );
    const allMaps = new Set(MAPS_PER_SERVER.flat());
    for (const name of allMaps) {
      const srvIdx = MAPS_PER_SERVER.findIndex((arr) => arr.includes(name));
      const v: Record<string, any> = {
        name, rounds: randInt(80, 500), kills: randInt(800, 4000),
        suicides: randInt(5, 80), teamkills: randInt(5, 50),
        server_id: srvIdx >= 0 ? SERVERS[srvIdx].id : 1,
      };
      try { stmt.run(...list.map((c) => v[c])); } catch {}
    }
    log(`seeded xlr_mapstats (${allMaps.size} maps)`);
  }

  if (tableExists(db, 'xlr_history')) {
    const h = columns(db, 'xlr_history');
    const list = ['client_id','kills','deaths','skill','time_add'].filter((c) => h.has(c));
    const placeholders = list.map((f) => f === 'time_add' ? "datetime('now', ?)" : '?').join(', ');
    const hstmt = db.prepare(`INSERT INTO xlr_history (${list.join(', ')}) VALUES (${placeholders})`);
    let ht = 0;
    for (let i = 0; i < FIRST_NAMES.length; i++) {
      const localRank = i % 20;
      if (localRank >= 15) continue;
      const cid = i + 1;
      const baseSkill = 1800 - localRank * 28;
      const baseKills = 60 - localRank * 2;
      for (let d = 29; d >= 0; d--) {
        const drift = randInt(-4, 4);
        const v: Record<string, any> = {
          client_id: cid,
          kills: Math.max(0, baseKills + drift),
          deaths: Math.max(0, randInt(20, 50) + drift),
          skill: +(baseSkill + drift * 3 - (29 - d) * 0.8).toFixed(1),
          time_add: `-${d} days`,
        };
        try { hstmt.run(...list.map((c) => v[c])); ht++; } catch {}
      }
    }
    log(`seeded ${ht} xlr_history rows`);
  }
}

function seedMapConfigs(db: Database.Database) {
  if (!tableExists(db, 'map_configs')) return;
  const cols = columns(db, 'map_configs');
  const hasServerId = cols.has('server_id');
  const fields = ['map_name','gametype','capturelimit','timelimit','fraglimit',
    'g_gear','g_friendlyfire','g_bombexplodetime','startmessage'].filter((c) => cols.has(c));
  if (hasServerId) fields.push('server_id');
  const stmt = db.prepare(`INSERT OR IGNORE INTO map_configs (${fields.join(', ')}) VALUES (${fields.map(() => '?').join(', ')})`);

  const configs: Array<Record<string, any>> = [
    { map_name: 'ut4_turnpike', gametype: 'ts',   fraglimit: 0,  timelimit: 15, capturelimit: 0, g_gear: '63', g_friendlyfire: 2, g_bombexplodetime: 40, startmessage: '^2Welcome to ^3Turnpike^7!', server_id: 1 },
    { map_name: 'ut4_uptown',   gametype: 'ctf',  fraglimit: 0,  timelimit: 20, capturelimit: 5, g_gear: '0',  g_friendlyfire: 1, g_bombexplodetime: 40, startmessage: '^2Uptown CTF^7 - good luck!', server_id: 1 },
    { map_name: 'ut4_kingdom',  gametype: 'bomb', fraglimit: 0,  timelimit: 10, capturelimit: 0, g_gear: '63', g_friendlyfire: 2, g_bombexplodetime: 35, startmessage: '^3Bomb mode^7 - no hk69', server_id: 1 },
    { map_name: 'ut4_casa',     gametype: 'tdm',  fraglimit: 50, timelimit: 20, capturelimit: 0, g_gear: '0',  g_friendlyfire: 1, g_bombexplodetime: 40, startmessage: '^2Casa TDM^7', server_id: 2 },
    { map_name: 'ut4_algiers',  gametype: 'ctf',  fraglimit: 0,  timelimit: 20, capturelimit: 5, g_gear: '0',  g_friendlyfire: 1, g_bombexplodetime: 40, startmessage: '^2Algiers CTF^7', server_id: 2 },
    { map_name: 'ut4_mandolin', gametype: 'ts',   fraglimit: 0,  timelimit: 15, capturelimit: 0, g_gear: '63', g_friendlyfire: 2, g_bombexplodetime: 40, startmessage: '^3Team Survivor^7 - one life!', server_id: 2 },
    { map_name: 'ut4_abbey',    gametype: 'tdm',  fraglimit: 50, timelimit: 20, capturelimit: 0, g_gear: '0',  g_friendlyfire: 1, g_bombexplodetime: 40, startmessage: '^2Abbey TDM^7', server_id: 3 },
    { map_name: 'ut4_prague',   gametype: 'bomb', fraglimit: 0,  timelimit: 15, capturelimit: 0, g_gear: '63', g_friendlyfire: 2, g_bombexplodetime: 35, startmessage: '^3Bomb - pros only^7', server_id: 3 },
    { map_name: 'ut4_ramelle',  gametype: 'ctf',  fraglimit: 0,  timelimit: 25, capturelimit: 7, g_gear: '0',  g_friendlyfire: 1, g_bombexplodetime: 40, startmessage: '^2Ramelle CTF^7', server_id: 3 },
  ];
  let total = 0;
  for (const cfg of configs) {
    try { stmt.run(...fields.map((f) => cfg[f] ?? 0)); total++; } catch {}
  }
  log(`seeded ${total} map_configs`);
}

function seedAdminUsers(db: Database.Database) {
  if (!tableExists(db, 'admin_users')) return;
  const stmt = db.prepare(`
    INSERT OR REPLACE INTO admin_users (username, password_hash, role, created_at, updated_at)
    VALUES (?, ?, ?, datetime('now'), datetime('now'))
  `);
  stmt.run(env.adminUser, bcrypt.hashSync(env.adminPass, 10), 'admin');
  stmt.run('moderator', bcrypt.hashSync('demo', 10), 'moderator');
  stmt.run('viewer',    bcrypt.hashSync('demo', 10), 'viewer');
  log(`seeded admin_users (login: ${env.adminUser} / ${env.adminPass})`);
}

function seedAuditLog(db: Database.Database) {
  if (!tableExists(db, 'audit_log')) return;
  const cols = columns(db, 'audit_log');
  const userCol = cols.has('admin_user_id') ? 'admin_user_id' : cols.has('user_id') ? 'user_id' : null;
  const actionCol = cols.has('action') ? 'action' : null;
  const detailsCol = cols.has('detail') ? 'detail' : cols.has('details') ? 'details' : cols.has('meta') ? 'meta' : null;
  const timeCol = cols.has('created_at') ? 'created_at' : cols.has('time_add') ? 'time_add' : null;
  if (!userCol || !actionCol || !timeCol) return;
  const hasServerId = cols.has('server_id');
  const parts: string[] = [userCol, actionCol];
  if (detailsCol) parts.push(detailsCol);
  if (hasServerId) parts.push('server_id');
  parts.push(timeCol);
  const placeholders = parts.map((c) => c === timeCol ? `datetime('now', ?)` : '?').join(', ');
  const stmt = db.prepare(`INSERT INTO audit_log (${parts.join(', ')}) VALUES (${placeholders})`);

  const actions: Array<[string, string]> = [
    ['login', '{"from":"10.0.0.12"}'],
    ['kick_player', '{"player":"FragLord","reason":"spawn kill"}'],
    ['ban_player', '{"player":"Xero","reason":"aimbot"}'],
    ['rcon_command', '{"cmd":"bigtext","arg":"Welcome"}'],
    ['update_config', '{"section":"adv","key":"interval"}'],
    ['mapcycle_update', '{"maps":["ut4_turnpike","ut4_uptown"]}'],
    ['disable_penalty', '{"penalty_id":7}'],
    ['change_map', '{"map":"ut4_turnpike"}'],
    ['restart_bot', '{}'],
    ['update_map_config', '{"map":"ut4_casa","fraglimit":60}'],
    ['server_cfg_save', '{"cvar":"sv_maxclients","value":"20"}'],
    ['add_admin_user', '{"username":"moderator"}'],
  ];
  let total = 0;
  for (const srv of SERVERS) {
    const count = randInt(12, 16);
    for (let i = 0; i < count; i++) {
      const [action, details] = actions[randInt(0, actions.length - 1)];
      const args: any[] = [randInt(1, 3), action];
      if (detailsCol) args.push(details);
      if (hasServerId) args.push(srv.id);
      args.push(`-${randInt(1, 720)} minutes`);
      try { stmt.run(...args); total++; } catch {}
    }
  }
  log(`seeded ${total} audit_log rows`);
}

// -------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------

function main() {
  fs.mkdirSync(paths.out, { recursive: true });
  if (fs.existsSync(paths.db)) fs.unlinkSync(paths.db);
  log(`creating fresh database at ${paths.db}`);
  const db = new Database(paths.db);
  db.pragma('journal_mode = WAL');
  db.pragma('foreign_keys = ON');

  applyMigrations(db);

  if (!tableExists(db, 'clients')) {
    throw new Error('clients table missing after migrations');
  }

  const clientServer = buildClientServerMap();

  seedServers(db);
  seedClients(db);
  seedAliases(db);
  seedPenalties(db, clientServer);
  seedChat(db, clientServer);
  seedVoteHistory(db);
  seedXlrstats(db, clientServer);
  seedMapConfigs(db);
  seedAdminUsers(db);
  seedAuditLog(db);

  db.close();
  log('db done.');

  writeDemoConfigs();
}

function outPath(p: string) { return p.replace(/\\/g, '/'); }

function writeDemoConfigs() {
  const dbPath = outPath(paths.db);
  const out = outPath(paths.out);
  const certsDir = outPath(path.join(paths.out, 'certs'));

  // Master config (listens on web 2727 + sync 9443).
  const masterToml = `# Auto-generated - master mode demo config
[referee]
bot_name = "R3 Master"
bot_prefix = "^2R3:^3"
database = "sqlite://${dbPath}"
logfile = "${out}/referee-master.log"
log_level = "info"

[server]
public_ip = "127.0.0.1"
port = 27960
rcon_password = "demo"
game_log = "${out}/mock-games-master.log"
delay = 1.0

[web]
enabled = true
bind_address = "127.0.0.1"
port = 2727
jwt_secret = "demo-jwt-secret-do-not-use-in-production-placeholder-32chars!!"

[update]
enabled = false
url = "http://127.0.0.1:8090"
channel = "beta"

[master]
bind_address = "127.0.0.1"
port = 9443
tls_cert = "${certsDir}/server.crt"
tls_key  = "${certsDir}/server.key"
ca_cert  = "${certsDir}/ca.crt"
ca_key   = "${certsDir}/ca.key"
quick_connect_enabled = false

[[plugins]]
name = "admin"
enabled = true
[plugins.settings]
warn_reason = "^7behave yourself"
max_warnings = 3

[[plugins]]
name = "welcome"
enabled = true

[[plugins]]
name = "adv"
enabled = true
[plugins.settings]
interval = 180

[[plugins]]
name = "xlrstats"
enabled = true

[[plugins]]
name = "stats"
enabled = true

[[plugins]]
name = "chatlogger"
enabled = true
`;

  // Standalone config - used when we fall back from master mode.
  const standaloneToml = `# Auto-generated - standalone demo config (master fallback)
[referee]
bot_name = "Referee-Demo"
bot_prefix = "^2R3:^3"
database = "sqlite://${dbPath}"
logfile = "${out}/referee-demo.log"
log_level = "info"

[server]
public_ip = "127.0.0.1"
port = 27960
rcon_password = "demo"
game_log = "${out}/mock-games.log"
delay = 1.0

[web]
enabled = true
bind_address = "127.0.0.1"
port = 2727
jwt_secret = "demo-jwt-secret-do-not-use-in-production-placeholder-32chars!!"

[update]
enabled = false
url = "http://127.0.0.1:8090"
channel = "beta"

[[plugins]]
name = "admin"
enabled = true
[plugins.settings]
warn_reason = "^7behave yourself"
max_warnings = 3

[[plugins]]
name = "welcome"
enabled = true

[[plugins]]
name = "adv"
enabled = true
[plugins.settings]
interval = 180

[[plugins]]
name = "xlrstats"
enabled = true

[[plugins]]
name = "stats"
enabled = true

[[plugins]]
name = "chatlogger"
enabled = true

[[plugins]]
name = "spamcontrol"
enabled = true

[[plugins]]
name = "censor"
enabled = true

[[plugins]]
name = "tk"
enabled = true
`;

  const clientToml = `# Auto-generated - client mode demo config
[referee]
bot_name = "R3 Client"
bot_prefix = "^2R3:^3"
database = "sqlite://${dbPath.replace('demo.db', 'client-cache.db')}"
logfile = "${out}/referee-client.log"
log_level = "info"

[server]
public_ip = "127.0.0.1"
port = 27961
rcon_ip = "127.0.0.1"
rcon_port = 27961
rcon_password = "demo"
game_log = "${out}/mock-games-client.log"
server_cfg_path = "${out}/server.cfg"
delay = 1.0

[web]
enabled = false

[update]
enabled = false
url = "http://127.0.0.1:8090"
channel = "beta"

[client]
master_url = "https://127.0.0.1:9443"
server_name = "R3 US East - Turnpike"
tls_cert = "${certsDir}/client.crt"
tls_key  = "${certsDir}/client.key"
ca_cert  = "${certsDir}/ca.crt"
sync_interval = 10
heartbeat_interval = 10

[[plugins]]
name = "admin"
enabled = true

[[plugins]]
name = "welcome"
enabled = true
`;

  fs.writeFileSync(paths.config, standaloneToml); // legacy default path
  fs.writeFileSync(path.join(paths.out, 'referee-demo-master.toml'), masterToml);
  fs.writeFileSync(path.join(paths.out, 'referee-demo-client.toml'), clientToml);
  fs.writeFileSync(path.join(paths.out, 'referee-demo-standalone.toml'), standaloneToml);

  // Demo server.cfg consumed by GET /api/v1/servers/:id/server-cfg. Keys here
  // must match entries in ui/src/lib/urt-cvars.js so the form view populates.
  const serverCfg = [
    '// Demo server.cfg - R3 US East - Turnpike',
    'set sv_hostname "^2R3 US East - ^7Turnpike"',
    'set sv_maxclients "16"',
    'set sv_privateClients "2"',
    'set sv_privatePassword ""',
    'set rconpassword "demo"',
    'set g_password ""',
    'set g_gametype "4"',
    'set g_friendlyfire "2"',
    'set g_gear "63"',
    'set g_warmup "20"',
    'set g_matchmode "0"',
    'set fraglimit "0"',
    'set timelimit "15"',
    'set capturelimit "8"',
    'set g_maxrounds "0"',
    'set sv_floodprotect "1"',
    'set sv_maxrate "25000"',
    'set sv_minping "0"',
    'set sv_maxping "0"',
    'set g_allowvote "1"',
    'exec mapcycle.txt',
    '',
  ].join('\n');
  fs.writeFileSync(path.join(paths.out, 'server.cfg'), serverCfg);

  for (const f of ['mock-games.log', 'mock-games-master.log', 'mock-games-client.log']) {
    const p = path.join(paths.out, f);
    if (!fs.existsSync(p)) fs.writeFileSync(p, '');
  }
  log('wrote demo configs: master, client, standalone');
}

main();
