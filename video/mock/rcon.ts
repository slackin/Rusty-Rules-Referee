/**
 * Minimal UrT-style RCON responder so the demo bot can call "status", "say",
 * "map", etc. without a real game server. Listens on UDP :27960.
 *
 * UrT RCON protocol (quake3): client sends "\xff\xff\xff\xffrcon <pass> <cmd>",
 * server responds with "\xff\xff\xff\xffprint\n<payload>".
 */
import dgram from 'node:dgram';

const PORT = parseInt(process.env.MOCK_RCON_PORT ?? '27960', 10);
const HEADER = Buffer.from([0xff, 0xff, 0xff, 0xff]);

function reply(payload: string): Buffer {
  return Buffer.concat([HEADER, Buffer.from('print\n' + payload, 'utf8')]);
}

// Names + GUIDs match the first 11 rows the seeder puts into the `clients`
// table (video/seed/seed.ts FIRST_NAMES[0..11], GUIDs GUID00000001..00000011),
// so penalties / chat / stats / player-profile pages reference the same people.
interface MockPlayer {
  slot: number;
  name: string;
  guid: string;
  ip: string;
  score: number;
  ping: number;
  team: 'RED' | 'BLUE' | 'SPECTATOR';
  kills: number;
  deaths: number;
  assists: number;
}

const PLAYERS: MockPlayer[] = [
  { slot: 0,  name: 'Sn1per',       guid: 'GUID00000001', ip: '10.0.0.12',  score: 42, ping: 48, team: 'RED',       kills: 42, deaths: 18, assists: 5 },
  { slot: 1,  name: 'GhostRider',   guid: 'GUID00000002', ip: '10.0.0.21',  score: 37, ping: 55, team: 'BLUE',      kills: 37, deaths: 22, assists: 8 },
  { slot: 2,  name: 'FragLord',     guid: 'GUID00000003', ip: '10.0.0.34',  score: 35, ping: 71, team: 'RED',       kills: 35, deaths: 24, assists: 3 },
  { slot: 3,  name: 'NoScope',      guid: 'GUID00000004', ip: '10.0.0.41',  score: 33, ping: 38, team: 'BLUE',      kills: 33, deaths: 25, assists: 6 },
  { slot: 4,  name: 'Vortex',       guid: 'GUID00000005', ip: '10.0.0.55',  score: 29, ping: 89, team: 'RED',       kills: 29, deaths: 28, assists: 4 },
  { slot: 5,  name: 'Banshee',      guid: 'GUID00000006', ip: '10.0.0.63',  score: 27, ping: 62, team: 'BLUE',      kills: 27, deaths: 26, assists: 7 },
  { slot: 6,  name: 'Reaper',       guid: 'GUID00000007', ip: '10.0.0.71',  score: 24, ping: 45, team: 'RED',       kills: 24, deaths: 30, assists: 2 },
  { slot: 7,  name: 'Kingpin',      guid: 'GUID00000008', ip: '10.0.0.82',  score: 22, ping: 58, team: 'BLUE',      kills: 22, deaths: 27, assists: 9 },
  { slot: 8,  name: 'Mystique',     guid: 'GUID00000009', ip: '10.0.0.94',  score: 19, ping: 74, team: 'RED',       kills: 19, deaths: 31, assists: 3 },
  { slot: 9,  name: 'Onyx',         guid: 'GUID00000010', ip: '10.0.0.101', score: 15, ping: 51, team: 'BLUE',      kills: 15, deaths: 29, assists: 5 },
  { slot: 10, name: 'PixelPusher',  guid: 'GUID00000011', ip: '10.0.0.112', score:  8, ping: 66, team: 'SPECTATOR', kills:  0, deaths:  0, assists: 0 },
];

function buildStatus(): string {
  const header =
    'map: ut4_turnpike\n' +
    'num score ping name            lastmsg address               qport rate\n' +
    '--- ----- ---- --------------- ------- --------------------- ----- -----\n';
  const rows = PLAYERS.map((p) => {
    const num = String(p.slot).padStart(3, ' ');
    const score = String(p.score).padStart(5, ' ');
    const ping = String(p.ping).padStart(4, ' ');
    const name = p.name.padEnd(15, ' ').slice(0, 15);
    const addr = `${p.ip}:27960`.padEnd(21, ' ');
    const qport = String(1024 + p.slot * 97).padStart(5, ' ');
    return `${num} ${score} ${ping} ${name}       0 ${addr} ${qport} 25000`;
  });
  return header + rows.join('\n') + '\n';
}

function buildPlayers(): string {
  // Format the bot recognises: "<slot>:<name> TEAM:<team> KILLS:<k> DEATHS:<d> ASSISTS:<a> PING:<p>"
  return (
    PLAYERS.map(
      (p) =>
        `${p.slot}:${p.name}  TEAM:${p.team}  KILLS:${p.kills}  DEATHS:${p.deaths}  ASSISTS:${p.assists}  PING:${p.ping}`,
    ).join('\n') + '\n'
  );
}

function buildDumpuser(slotStr: string): string {
  const slot = parseInt(slotStr, 10);
  const p = PLAYERS.find((x) => x.slot === slot);
  if (!p) {
    return 'userinfo\n--------\n';
  }
  return [
    'userinfo',
    '--------',
    `ip                  ${p.ip}:27960`,
    `name                ${p.name}`,
    `cl_guid             ${p.guid}`,
    `team                ${p.team}`,
    `cg_rgb              255 255 255`,
    '',
  ].join('\n');
}

// UrT fdir format: path/to/file on its own line, terminated by "<N> files listed"
const MAP_LIST = [
  'ut4_turnpike', 'ut4_uptown',   'ut4_kingdom',
  'ut4_casa',     'ut4_algiers',  'ut4_mandolin',
  'ut4_abbey',    'ut4_prague',   'ut4_ramelle',
];

function buildFdir(): string {
  const lines = ['---------------', ...MAP_LIST.map((m) => `maps/${m}.bsp`), `${MAP_LIST.length} files listed`];
  return lines.join('\n') + '\n';
}

function handle(cmd: string): string {
  const trimmed = cmd.trim();
  if (!trimmed) return '';
  const [verb, ...rest] = trimmed.split(/\s+/);
  const args = rest.join(' ');
  switch (verb.toLowerCase()) {
    case 'status':
      return buildStatus();
    case 'players':
      return buildPlayers();
    case 'dumpuser':
      return buildDumpuser(args);
    case 'fdir':
      return buildFdir();
    case 'say':
    case 'bigtext':
      return `broadcast: ${args}`;
    case 'map':
      return `"map" is: "${args || 'ut4_turnpike'}"\nsv_pure is:"1"\n`;
    case 'cvarlist':
      return 'g_gametype "4"\nsv_maxclients "16"\nmapname "ut4_turnpike"\n';
    case 'kick':
    case 'clientkick':
      return `kicked ${args}`;
    default:
      return `unknown cmd: ${verb}`;
  }
}

const sock = dgram.createSocket('udp4');

sock.on('message', (msg, rinfo) => {
  // Strip 4 leading 0xFF bytes
  if (msg.length < 5 || msg[0] !== 0xff || msg[1] !== 0xff || msg[2] !== 0xff || msg[3] !== 0xff) return;
  const body = msg.slice(4).toString('utf8');
  // Expected: "rcon <password> <command>"
  const m = body.match(/^rcon\s+\S+\s+([\s\S]*)$/);
  if (!m) return;
  const response = handle(m[1]);
  sock.send(reply(response), rinfo.port, rinfo.address);
});

sock.on('listening', () => {
  const a = sock.address();
  console.log(`[mock-rcon] listening on ${a.address}:${a.port}`);
});

sock.on('error', (err) => {
  console.error('[mock-rcon] error:', err.message);
});

sock.bind(PORT, '127.0.0.1');

process.on('SIGINT', () => { sock.close(); process.exit(0); });
process.on('SIGTERM', () => { sock.close(); process.exit(0); });
