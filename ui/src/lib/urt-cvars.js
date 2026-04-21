// Urban Terror 4.3 cvar registry used by the server.cfg editor.
//
// Entries are roughly ordered as they commonly appear in server.cfg.
// The editor preserves unknown cvars, comments, binds, and exec lines
// verbatim, so this registry can be best-effort.
//
// Types:
//   string       — free-form text
//   int / float  — numeric
//   bool01       — toggle, serialized as "0"/"1"
//   enum         — select (uses `options: [{value,label}]`)
//   gear         — uses the gear picker
//   bitmask      — checkbox grid (uses `flags: [{value,label}]`)

export const SECTIONS = [
	{ id: 'identity', title: 'Server Identity', defaultOpen: true },
	{ id: 'network', title: 'Networking & Slots', defaultOpen: true },
	{ id: 'security', title: 'Security & Admin', defaultOpen: true },
	{ id: 'rules', title: 'Game Rules', defaultOpen: true },
	{ id: 'teamplay', title: 'Team Play', defaultOpen: false },
	{ id: 'voting', title: 'Voting', defaultOpen: false },
	{ id: 'weapons', title: 'Weapons & Gear', defaultOpen: false },
	{ id: 'matchmodes', title: 'Match Modes (CTF / Bomb / CAH)', defaultOpen: false },
	{ id: 'physics', title: 'Movement & Physics', defaultOpen: false },
	{ id: 'bots', title: 'Bots', defaultOpen: false },
	{ id: 'logging', title: 'Logging & Stats', defaultOpen: false },
	{ id: 'mapcycle', title: 'Mapcycle & Maps', defaultOpen: false },
];

// Gear items — same codes/order as the map-config editor.
export const GEAR_ITEMS = [
	{ code: 'G', label: 'Grenades' },
	{ code: 'A', label: 'Snipers (SR-8, PSG-1)' },
	{ code: 'a', label: 'Negev' },
	{ code: 'I', label: 'SMGs (MP5K, UMP45, MAC-11)' },
	{ code: 'W', label: 'Pistols (Desert Eagle, .50)' },
	{ code: 'N', label: 'Pistols (Beretta, Colt 1911)' },
	{ code: 'E', label: 'Automatics (G36, AK-103, LR300)' },
	{ code: 'M', label: 'Shotguns (SPAS-12, Benelli)' },
	{ code: 'K', label: 'Kevlar Vest' },
	{ code: 'L', label: 'Laser Sight' },
	{ code: 'O', label: 'Medkit' },
	{ code: 'Q', label: 'Silencer' },
	{ code: 'R', label: 'Extra Ammo' },
	{ code: 'S', label: 'Helmet' },
	{ code: 'T', label: 'NVGs (Night Vision)' },
	{ code: 'U', label: 'Tactical Goggles' },
	{ code: 'V', label: 'HE Grenade' },
	{ code: 'X', label: 'Smoke Grenade' },
	{ code: 'Z', label: 'HK69 Grenade Launcher' },
];

// g_allowvote bit flags (UrT 4.3).
export const ALLOWVOTE_FLAGS = [
	{ value: 1, label: 'restart', help: 'restart' },
	{ value: 2, label: 'cyclemap', help: 'cyclemap' },
	{ value: 4, label: 'map', help: 'map <name>' },
	{ value: 8, label: 'g_gametype', help: 'change gametype' },
	{ value: 16, label: 'kick', help: 'kick player' },
	{ value: 32, label: 'clientkick', help: 'clientkick by slot' },
	{ value: 64, label: 'g_doWarmup', help: 'toggle warmup' },
	{ value: 128, label: 'timelimit', help: 'set timelimit' },
	{ value: 256, label: 'fraglimit', help: 'set fraglimit' },
	{ value: 512, label: 'exec', help: 'exec config' },
	{ value: 1024, label: 'shuffleteams', help: 'shuffle teams' },
	{ value: 2048, label: 'nextmap', help: 'nextmap' },
	{ value: 4096, label: 'mute', help: 'mute player' },
	{ value: 8192, label: 'capturelimit', help: 'set capturelimit' },
	{ value: 16384, label: 'surrender', help: 'surrender (BOMB)' },
	{ value: 32768, label: 'swapteams', help: 'swap teams' },
	{ value: 65536, label: 'nuke', help: 'nuke' },
	{ value: 131072, label: 'reload', help: 'force reload' },
	{ value: 262144, label: 'referee', help: 'referee' },
];

// Gametype enum.
const GAMETYPES = [
	{ value: '0', label: '0 — Free For All (FFA)' },
	{ value: '1', label: '1 — Last Man Standing (LMS)' },
	{ value: '3', label: '3 — Team Death Match (TDM)' },
	{ value: '4', label: '4 — Team Survivor (TS)' },
	{ value: '5', label: '5 — Follow the Leader (FTL)' },
	{ value: '6', label: '6 — Capture & Hold (CAH)' },
	{ value: '7', label: '7 — Capture the Flag (CTF)' },
	{ value: '8', label: '8 — Bomb Mode (BOMB)' },
	{ value: '9', label: '9 — Jump Mode' },
	{ value: '10', label: '10 — Freeze Tag (FT)' },
	{ value: '11', label: '11 — Gun Game' },
];

const FRIENDLYFIRE = [
	{ value: '0', label: '0 — Off' },
	{ value: '1', label: '1 — On' },
	{ value: '2', label: '2 — Mirror (reflect)' },
	{ value: '3', label: '3 — Shared (pool)' },
];

// Flat cvar list. Order determines display order within a section.
export const CVARS = [
	// ---- identity ----
	{ key: 'sv_hostname', section: 'identity', type: 'string', label: 'Hostname', default: 'Urban Terror Server',
	  help: 'Name shown in the server browser. Supports ^<n> color codes.' },
	{ key: 'sv_joinmessage', section: 'identity', type: 'string', label: 'Join message', default: '',
	  help: 'Message shown to players as they connect.' },
	{ key: 'sv_motd', section: 'identity', type: 'string', label: 'MOTD', default: '',
	  help: 'Message of the day, shown once connected.' },
	{ key: 'sv_keywords', section: 'identity', type: 'string', label: 'Keywords', default: '',
	  help: 'Space-separated keywords for the master server browser filter.' },
	{ key: '_Admin', section: 'identity', type: 'string', label: 'Admin name', default: '',
	  help: 'Cosmetic: admin/contact info shown via /admins.', advanced: true },
	{ key: '_Email', section: 'identity', type: 'string', label: 'Admin email', default: '', advanced: true },
	{ key: '_Location', section: 'identity', type: 'string', label: 'Location', default: '', advanced: true },
	{ key: '_URT_Server', section: 'identity', type: 'string', label: 'URT server banner', default: '', advanced: true },

	// ---- network ----
	{ key: 'net_port', section: 'network', type: 'int', label: 'Port', default: 27960, min: 1, max: 65535,
	  help: 'UDP port the game server listens on.' },
	{ key: 'net_ip', section: 'network', type: 'string', label: 'Bind IP', default: '0.0.0.0',
	  help: 'Interface to bind. 0.0.0.0 = all.' , advanced: true },
	{ key: 'sv_maxclients', section: 'network', type: 'int', label: 'Max clients', default: 16, min: 1, max: 64,
	  help: 'Total player slots (including private).' },
	{ key: 'sv_privateClients', section: 'network', type: 'int', label: 'Private (reserved) slots', default: 0, min: 0, max: 64,
	  help: 'Slots reserved for players who know sv_privatePassword.' },
	{ key: 'sv_privatePassword', section: 'network', type: 'string', label: 'Private slot password', default: '',
	  help: 'Password required to use a private slot.' },
	{ key: 'g_password', section: 'network', type: 'string', label: 'Server password', default: '',
	  help: 'Leave empty for a public server.' },
	{ key: 'sv_maxPing', section: 'network', type: 'int', label: 'Max ping (ms)', default: 0, min: 0, max: 999,
	  help: '0 = no limit. Players exceeding this ping will be kicked.' },
	{ key: 'sv_minPing', section: 'network', type: 'int', label: 'Min ping (ms)', default: 0, min: 0, max: 999, advanced: true },
	{ key: 'sv_maxRate', section: 'network', type: 'int', label: 'Max rate (bytes/s)', default: 25000, min: 0,
	  help: 'Per-client network rate cap. Common: 25000.' },
	{ key: 'sv_dlRate', section: 'network', type: 'int', label: 'HTTP download rate (KB/s)', default: 100, min: 0, advanced: true },
	{ key: 'sv_allowdownload', section: 'network', type: 'int', label: 'Allow download mode', default: 0,
	  help: 'Bitmask: 0=off, 1=UDP download, 2+ = HTTP redirect options. Common production value: 2.',
	  advanced: true },
	{ key: 'sv_dlURL', section: 'network', type: 'string', label: 'HTTP download URL', default: '',
	  help: 'Root URL where clients can fetch maps/assets (sv_allowdownload must allow HTTP).' },
	{ key: 'sv_pure', section: 'network', type: 'bool01', label: 'Pure server', default: '1',
	  help: 'Require clients to use only server-approved .pk3 files.' },
	{ key: 'sv_fps', section: 'network', type: 'int', label: 'Server FPS', default: 20, min: 10, max: 125, advanced: true,
	  help: 'Simulation rate. 20 is stock; higher for competitive.' },
	{ key: 'sv_floodProtect', section: 'network', type: 'bool01', label: 'Flood protect', default: '1' },
	{ key: 'sv_reconnectlimit', section: 'network', type: 'int', label: 'Reconnect limit (s)', default: 3, min: 0, advanced: true },
	{ key: 'sv_timeout', section: 'network', type: 'int', label: 'Client timeout (s)', default: 200, min: 0, advanced: true },
	{ key: 'sv_zombietime', section: 'network', type: 'int', label: 'Zombie time (s)', default: 2, min: 0, advanced: true },
	{ key: 'sv_clientsPerIp', section: 'network', type: 'int', label: 'Clients per IP', default: 3, min: 1, max: 32 },

	// ---- security ----
	{ key: 'rconPassword', section: 'security', type: 'string', label: 'RCON password', default: '',
	  help: 'Administrative password. Required by R3.' },
	{ key: 'sv_allowAnyRconCommand', section: 'security', type: 'bool01', label: 'Allow any RCON command', default: '0', advanced: true },
	{ key: 'g_banIPs', section: 'security', type: 'string', label: 'Banned IPs', default: '',
	  help: 'Space-separated list of banned IPs.' , advanced: true },
	{ key: 'auth_enable', section: 'security', type: 'bool01', label: 'Auth system enabled', default: '0',
	  help: 'Enables FrozenSand authentication (account system).' },
	{ key: 'auth_notoriety', section: 'security', type: 'int', label: 'Auth notoriety', default: 0, advanced: true },
	{ key: 'auth_owners', section: 'security', type: 'string', label: 'Auth owners', default: '',
	  help: 'Auth account IDs with server-owner privileges.' , advanced: true },
	{ key: 'auth_tags', section: 'security', type: 'string', label: 'Auth tags', default: '', advanced: true },
	{ key: 'auth_cheaters', section: 'security', type: 'int', label: 'Kick known cheaters', default: 0, advanced: true },
	{ key: 'auth_log', section: 'security', type: 'bool01', label: 'Auth log', default: '0', advanced: true },
	{ key: 'auth_verbosity', section: 'security', type: 'int', label: 'Auth verbosity', default: 1, advanced: true },
	{ key: 'auth_status_message_time', section: 'security', type: 'int', label: 'Auth status msg time (s)', default: 5, advanced: true },

	// ---- rules ----
	{ key: 'g_gametype', section: 'rules', type: 'enum', label: 'Gametype', default: '7', options: GAMETYPES },
	{ key: 'timelimit', section: 'rules', type: 'int', label: 'Time limit (min)', default: 20, min: 0 },
	{ key: 'fraglimit', section: 'rules', type: 'int', label: 'Frag limit', default: 0, min: 0 },
	{ key: 'capturelimit', section: 'rules', type: 'int', label: 'Capture limit', default: 9, min: 0 },
	{ key: 'g_maxrounds', section: 'rules', type: 'int', label: 'Max rounds (TS/FT/BOMB)', default: 12, min: 0 },
	{ key: 'g_friendlyfire', section: 'rules', type: 'enum', label: 'Friendly fire', default: '0', options: FRIENDLYFIRE },
	{ key: 'g_warmup', section: 'rules', type: 'int', label: 'Warmup duration (s)', default: 10, min: 0 },
	{ key: 'g_doWarmup', section: 'rules', type: 'bool01', label: 'Do warmup', default: '1' },
	{ key: 'g_matchmode', section: 'rules', type: 'bool01', label: 'Match mode', default: '0',
	  help: 'Competitive mode: locks teams, requires /ready.' },
	{ key: 'g_suddendeath', section: 'rules', type: 'bool01', label: 'Sudden death', default: '1', advanced: true },
	{ key: 'g_nextmap', section: 'rules', type: 'string', label: 'Next map (override)', default: '', advanced: true },

	// ---- teamplay ----
	{ key: 'g_teamnamered', section: 'teamplay', type: 'string', label: 'Red team name', default: 'Red' },
	{ key: 'g_teamnameblue', section: 'teamplay', type: 'string', label: 'Blue team name', default: 'Blue' },
	{ key: 'g_autoteam', section: 'teamplay', type: 'bool01', label: 'Auto team (balance join)', default: '0' },
	{ key: 'g_teamforcebalance', section: 'teamplay', type: 'bool01', label: 'Force team balance', default: '0' },
	{ key: 'g_balancedteams', section: 'teamplay', type: 'bool01', label: 'Balanced teams', default: '0', advanced: true },
	{ key: 'g_swaproles', section: 'teamplay', type: 'bool01', label: 'Swap roles (TS/BOMB halftime)', default: '1' },
	{ key: 'g_autojoin', section: 'teamplay', type: 'bool01', label: 'Auto join (FFA autostart)', default: '0', advanced: true },
	{ key: 'g_armbands', section: 'teamplay', type: 'int', label: 'Armbands visibility', default: 0, advanced: true },
	{ key: 'g_teamkillsforfeit', section: 'teamplay', type: 'int', label: 'TK forfeit count', default: 0, advanced: true },
	{ key: 'g_cuff', section: 'teamplay', type: 'bool01', label: 'Allow cuff (radio)', default: '0', advanced: true },

	// ---- voting ----
	{ key: 'g_allowvote', section: 'voting', type: 'bitmask', label: 'Allowed votes', default: 0, flags: ALLOWVOTE_FLAGS,
	  help: 'Which vote types players may call. Sum of bit flags.' },
	{ key: 'g_voteTimelimit', section: 'voting', type: 'int', label: 'Vote timeout (s)', default: 30, min: 5, max: 300 },
	{ key: 'g_minVoters', section: 'voting', type: 'int', label: 'Minimum voters', default: 0, min: 0, advanced: true },
	{ key: 'g_voteMinPlayers', section: 'voting', type: 'int', label: 'Min players for votes', default: 0, min: 0, advanced: true },
	{ key: 'g_voteShuffleMode', section: 'voting', type: 'int', label: 'Shuffle-vote mode', default: 0, advanced: true },

	// ---- weapons ----
	{ key: 'g_gear', section: 'weapons', type: 'gear', label: 'Allowed weapons & gear', default: '',
	  help: 'Use the picker to build the string. Empty = all allowed.' },
	{ key: 'g_knifeDamageMult', section: 'weapons', type: 'float', label: 'Knife damage multiplier', default: 1.0, min: 0, advanced: true },
	{ key: 'g_allowweaponreload', section: 'weapons', type: 'bool01', label: 'Allow weapon reload', default: '1', advanced: true },
	{ key: 'g_suicide', section: 'weapons', type: 'bool01', label: 'Allow /kill', default: '1' },
	{ key: 'g_deadchat', section: 'weapons', type: 'bool01', label: 'Dead can chat', default: '0' },
	{ key: 'g_hitmessages', section: 'weapons', type: 'bool01', label: 'Hit messages', default: '1', advanced: true },
	{ key: 'g_respawnProtection', section: 'weapons', type: 'int', label: 'Respawn protection (ms)', default: 0, min: 0, advanced: true },
	{ key: 'g_respawndelay', section: 'weapons', type: 'int', label: 'Respawn delay (s)', default: 3, min: 0 },
	{ key: 'g_waverespawns', section: 'weapons', type: 'bool01', label: 'Wave respawns', default: '0', advanced: true },
	{ key: 'g_inactivity', section: 'weapons', type: 'int', label: 'Inactivity kick (s)', default: 0, min: 0 },
	{ key: 'g_antilag', section: 'weapons', type: 'bool01', label: 'Anti-lag', default: '1', advanced: true },
	{ key: 'g_antilagvis', section: 'weapons', type: 'bool01', label: 'Anti-lag visualization', default: '0', advanced: true },
	{ key: 'g_maxGameClients', section: 'weapons', type: 'int', label: 'Max game clients (non-spec)', default: 0, min: 0, advanced: true },

	// ---- matchmodes ----
	{ key: 'g_bombexplodetime', section: 'matchmodes', type: 'int', label: 'Bomb explode time (s)', default: 40, min: 1 },
	{ key: 'g_bombdefusetime', section: 'matchmodes', type: 'int', label: 'Bomb defuse time (s)', default: 10, min: 1 },
	{ key: 'g_bombPlantTime', section: 'matchmodes', type: 'int', label: 'Bomb plant time (s)', default: 3, min: 1, advanced: true },
	{ key: 'g_ctf_enableflagtoss', section: 'matchmodes', type: 'bool01', label: 'CTF: enable flag toss', default: '1' },
	{ key: 'g_ctf_instantcaps', section: 'matchmodes', type: 'bool01', label: 'CTF: instant caps', default: '0', advanced: true },
	{ key: 'g_ctf_dropflag', section: 'matchmodes', type: 'bool01', label: 'CTF: drop flag on death', default: '1', advanced: true },
	{ key: 'g_cahtime', section: 'matchmodes', type: 'int', label: 'CAH round time (min)', default: 4, min: 1 },
	{ key: 'g_followstrict', section: 'matchmodes', type: 'bool01', label: 'Strict follow (spectator)', default: '0' },
	{ key: 'g_flagmessages', section: 'matchmodes', type: 'bool01', label: 'CTF flag messages', default: '1', advanced: true },
	{ key: 'g_survivor', section: 'matchmodes', type: 'int', label: 'Survivor mode', default: 0, advanced: true },

	// ---- physics ----
	{ key: 'g_gravity', section: 'physics', type: 'int', label: 'Gravity', default: 800 },
	{ key: 'g_speed', section: 'physics', type: 'int', label: 'Player speed', default: 320 },
	{ key: 'g_knockback', section: 'physics', type: 'int', label: 'Knockback', default: 1000, advanced: true },
	{ key: 'g_stamina', section: 'physics', type: 'int', label: 'Stamina regen rate', default: 1, advanced: true },
	{ key: 'g_walljumps', section: 'physics', type: 'int', label: 'Wall jumps (count)', default: 3, min: 0, max: 99 },
	{ key: 'g_stanceFallDamage', section: 'physics', type: 'bool01', label: 'Stance fall damage', default: '1', advanced: true },

	// ---- bots ----
	{ key: 'bot_enable', section: 'bots', type: 'bool01', label: 'Enable bots', default: '0' },
	{ key: 'bot_minplayers', section: 'bots', type: 'int', label: 'Fill with bots up to', default: 0, min: 0, max: 64 },
	{ key: 'bot_skill', section: 'bots', type: 'int', label: 'Bot skill (1-5)', default: 3, min: 1, max: 5 },
	{ key: 'bot_nochat', section: 'bots', type: 'bool01', label: 'Silence bot chat', default: '1', advanced: true },
	{ key: 'bot_thinktime', section: 'bots', type: 'int', label: 'Bot think time (ms)', default: 100, advanced: true },

	// ---- logging ----
	{ key: 'g_log', section: 'logging', type: 'string', label: 'Game log filename', default: 'games.log',
	  help: 'Required by R3 to read server events.' },
	{ key: 'g_logsync', section: 'logging', type: 'bool01', label: 'Sync log (flush each line)', default: '1',
	  help: 'Required by R3 for real-time event parsing.' },
	{ key: 'g_loghits', section: 'logging', type: 'bool01', label: 'Log hits', default: '1' },
	{ key: 'g_logRoll', section: 'logging', type: 'bool01', label: 'Roll log on size', default: '0', advanced: true },
	{ key: 'g_logRollLength', section: 'logging', type: 'int', label: 'Log roll length (lines)', default: 0, advanced: true },
	{ key: 'logfile', section: 'logging', type: 'int', label: 'Engine logfile', default: 1, min: 0, max: 3, advanced: true,
	  help: '0=off, 1=buffered, 2=sync, 3=append.' },

	// ---- mapcycle ----
	{ key: 'g_mapcycle', section: 'mapcycle', type: 'string', label: 'Mapcycle file', default: 'mapcycle.txt',
	  help: 'Filename (relative to q3ut4/) listing the rotation. Use the Mapcycle tab to edit contents.' },
	{ key: 'sv_mapRotation', section: 'mapcycle', type: 'string', label: 'sv_mapRotation (alt)', default: '', advanced: true },
	{ key: 'g_delagHitscan', section: 'mapcycle', type: 'bool01', label: 'Delag hitscan', default: '1', advanced: true },
];

// Lookup helpers
const BY_KEY = Object.fromEntries(CVARS.map((c) => [c.key, c]));
export function getCvar(key) { return BY_KEY[key] || null; }

export function cvarsForSection(sectionId, { includeAdvanced }) {
	return CVARS.filter((c) => c.section === sectionId && (includeAdvanced || !c.advanced));
}
