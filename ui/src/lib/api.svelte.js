const BASE = '/api/v1';

/** @type {string|null} */
let token = $state(null);

export function setToken(t) {
	token = t;
	if (t) {
		localStorage.setItem('r3_token', t);
	} else {
		localStorage.removeItem('r3_token');
	}
}

export function getToken() {
	if (!token) {
		token = localStorage.getItem('r3_token');
	}
	return token;
}

/**
 * @param {string} path
 * @param {RequestInit} [opts]
 */
async function request(path, opts = {}) {
	const t = getToken();
	const headers = { 'Content-Type': 'application/json', ...opts.headers };
	if (t) headers['Authorization'] = `Bearer ${t}`;

	const res = await fetch(`${BASE}${path}`, { ...opts, headers });

	if (res.status === 401) {
		setToken(null);
		window.location.href = '/login';
		throw new Error('Unauthorized');
	}

	if (!res.ok) {
		const text = await res.text();
		throw new Error(text || res.statusText);
	}

	const ct = res.headers.get('content-type');
	if (ct && ct.includes('application/json')) {
		return res.json();
	}
	return res.text();
}

export const api = {
	// Auth
	login: (username, password) =>
		request('/auth/login', { method: 'POST', body: JSON.stringify({ username, password }) }),
	me: () => request('/auth/me'),

	// Server
	serverStatus: () => request('/server/status'),
	rcon: (command) =>
		request('/server/rcon', { method: 'POST', body: JSON.stringify({ command }) }),
	say: (message) =>
		request('/server/say', { method: 'POST', body: JSON.stringify({ message }) }),
	mapList: () => request('/server/maps'),
	refreshMaps: () => request('/server/maps/refresh', { method: 'POST' }),
	changeMap: (map, action) =>
		request('/server/map', { method: 'POST', body: JSON.stringify({ map, action }) }),
	restartBot: () =>
		request('/server/restart', { method: 'POST' }),
	mapcycle: () => request('/server/mapcycle'),
	updateMapcycle: (maps) =>
		request('/server/mapcycle', { method: 'PUT', body: JSON.stringify({ maps }) }),
	getCvar: (name) => request(`/server/cvar/${encodeURIComponent(name)}`),
	setCvar: (name, value) =>
		request(`/server/cvar/${encodeURIComponent(name)}`, { method: 'PUT', body: JSON.stringify({ value }) }),

	// Map configs (per-map settings)
	mapConfigs: () => request('/map-configs').then(r => r.configs),
	mapConfig: (id) => request(`/map-configs/${id}`),
	createMapConfig: (config) =>
		request('/map-configs', { method: 'POST', body: JSON.stringify(config) }),
	updateMapConfig: (id, config) =>
		request(`/map-configs/${id}`, { method: 'PUT', body: JSON.stringify(config) }),
	deleteMapConfig: (id) =>
		request(`/map-configs/${id}`, { method: 'DELETE' }),

	// Players
	players: () => request('/players').then(r => r.players),
	player: (id) => request(`/players/${id}`),
	kickPlayer: (cid, reason) =>
		request(`/players/${cid}/kick`, { method: 'POST', body: JSON.stringify({ reason }) }),
	banPlayer: (cid, reason, duration) =>
		request(`/players/${cid}/ban`, { method: 'POST', body: JSON.stringify({ reason, duration }) }),
	messagePlayer: (cid, message) =>
		request(`/players/${cid}/message`, { method: 'POST', body: JSON.stringify({ message }) }),
	mutePlayer: (cid, duration, reason) =>
		request(`/players/${cid}/mute`, { method: 'POST', body: JSON.stringify({ reason, duration }) }),
	unmutePlayer: (cid) =>
		request(`/players/${cid}/unmute`, { method: 'POST' }),
	searchClients: (query) => request(`/clients/search?q=${encodeURIComponent(query)}`).then(r => r.clients),
	allClients: ({ limit = 25, offset = 0, search = '', sortBy = 'last_visit', order = 'desc' } = {}) => {
		let url = `/clients?limit=${limit}&offset=${offset}&sort_by=${sortBy}&order=${order}`;
		if (search) url += `&search=${encodeURIComponent(search)}`;
		return request(url);
	},
	updatePlayerGroup: (id, groupId) =>
		request(`/players/${id}/group`, { method: 'PUT', body: JSON.stringify({ group_id: groupId }) }),

	// Penalties
	penalties: (params = '') => request(`/penalties${params ? '?' + params : ''}`),
	disablePenalty: (id) => request(`/penalties/${id}/disable`, { method: 'POST' }),

	// Groups
	groups: () => request('/groups').then(r => r.groups),

	// Aliases
	aliases: (clientId) => request(`/aliases?client_id=${clientId}`),

	// Config
	getConfig: () => request('/config'),
	updateConfig: (config) =>
		request('/config', { method: 'PUT', body: JSON.stringify(config) }),
	migrateToMysql: (params) =>
		request('/config/migrate-to-mysql', { method: 'POST', body: JSON.stringify(params) }),
	analyzeServerCfg: (path) =>
		request('/config/server-cfg', { method: 'POST', body: JSON.stringify({ path }) }),
	saveServerCfg: (path, content) =>
		request('/config/server-cfg/save', { method: 'POST', body: JSON.stringify({ path, content }) }),
	browseFiles: (path) =>
		request('/config/browse', { method: 'POST', body: JSON.stringify({ path }) }),

	// Plugins
	plugins: () => request('/plugins'),

	// Stats
	leaderboard: (limit = 25, offset = 0) =>
		request(`/stats/leaderboard?limit=${limit}&offset=${offset}`).then(r => r.leaderboard),
	playerStats: (id) => request(`/stats/player/${id}`),
	weaponStats: () => request('/stats/weapons').then(r => r.weapons),
	mapStats: () => request('/stats/maps').then(r => r.maps),
	dashboardSummary: () => request('/stats/summary'),

	// Chat
	chat: (limit = 50, beforeId = null) =>
		request(`/chat?limit=${limit}${beforeId ? '&before_id=' + beforeId : ''}`).then(r => r.messages),
	searchChat: ({ limit = 100, beforeId = null, query = '', clientId = null } = {}) => {
		let url = `/chat?limit=${limit}`;
		if (beforeId) url += `&before_id=${beforeId}`;
		if (query) url += `&query=${encodeURIComponent(query)}`;
		if (clientId) url += `&client_id=${clientId}`;
		return request(url).then(r => r.messages);
	},

	// Votes
	votes: (limit = 20) =>
		request(`/votes?limit=${limit}`).then(r => r.votes),

	// Commands documentation
	commands: () => request('/commands').then(r => r.commands),

	// Notes
	notes: () => request('/notes').then(r => r.content),
	saveNotes: (content) =>
		request('/notes', { method: 'PUT', body: JSON.stringify({ content }) }),

	// Audit log
	auditLog: (limit = 50, offset = 0) =>
		request(`/audit-log?limit=${limit}&offset=${offset}`).then(r => r.entries),

	// Admin Users
	users: () => request('/users').then(r => r.users),
	createUser: (user) =>
		request('/users', { method: 'POST', body: JSON.stringify(user) }),
	updateUser: (id, user) =>
		request(`/users/${id}`, { method: 'PUT', body: JSON.stringify(user) }),
	deleteUser: (id) => request(`/users/${id}`, { method: 'DELETE' }),
	changePassword: (currentPassword, newPassword) =>
		request('/users/me/password', { method: 'PUT', body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }) }),

	// Version & Updates
	version: () => request('/version'),
	checkUpdate: () => request('/version/check', { method: 'POST' }),
	applyUpdate: () => request('/version/update', { method: 'POST' }),

	// Setup
	setupStatus: () => request('/setup/status'),
	completeSetup: (data) =>
		request('/setup/complete', { method: 'POST', body: JSON.stringify(data) }),
	setupBrowse: (path) =>
		request('/setup/browse', { method: 'POST', body: JSON.stringify({ path }) }),
	setupScanConfigs: () =>
		request('/setup/scan-configs', { method: 'POST' }),
	setupAnalyzeCfg: (path) =>
		request('/setup/analyze-cfg', { method: 'POST', body: JSON.stringify({ path }) }),

	// Multi-server management (master mode)
	servers: () => request('/servers').then(r => r.servers),
	server: (id) => request(`/servers/${id}`),
	deleteServer: (id) => request(`/servers/${id}`, { method: 'DELETE' }),
	serverRcon: (id, command) =>
		request(`/servers/${id}/rcon`, { method: 'POST', body: JSON.stringify({ command }) }),
	serverKick: (id, cid, reason) =>
		request(`/servers/${id}/kick`, { method: 'POST', body: JSON.stringify({ cid, reason }) }),
	serverBan: (id, cid, reason, duration_minutes) =>
		request(`/servers/${id}/ban`, { method: 'POST', body: JSON.stringify({ cid, reason, duration_minutes }) }),
	serverSay: (id, message) =>
		request(`/servers/${id}/say`, { method: 'POST', body: JSON.stringify({ message }) }),
	serverMessage: (id, cid, message) =>
		request(`/servers/${id}/message`, { method: 'POST', body: JSON.stringify({ cid, message }) }),
	serverConfig: (id) => request(`/servers/${id}/config`),
	updateServerConfig: (id, config) =>
		request(`/servers/${id}/config`, { method: 'PUT', body: JSON.stringify(config) }),

	// Server setup (config scan, install, browse)
	scanServerConfigs: (id) =>
		request(`/servers/${id}/scan-configs`, { method: 'POST' }),
	parseServerConfig: (id, path) =>
		request(`/servers/${id}/parse-config`, { method: 'POST', body: JSON.stringify({ path }) }),
	browseServerFiles: (id, path = '') =>
		request(`/servers/${id}/browse`, { method: 'POST', body: JSON.stringify({ path }) }),
	installGameServer: (id, install_path) =>
		request(`/servers/${id}/install-server`, { method: 'POST', body: JSON.stringify({ install_path }) }),
	installStatus: (id) =>
		request(`/servers/${id}/install-status`),
	// UrT install wizard (Phase 6)
	wizardSuggest: (id) => request(`/servers/${id}/wizard/suggest`),
	wizardProbePorts: (id, ports, kind = 'udp') =>
		request(`/servers/${id}/wizard/probe-ports`, {
			method: 'POST',
			body: JSON.stringify({ ports, kind })
		}),
	wizardInstall: (id, params) =>
		request(`/servers/${id}/wizard/install`, {
			method: 'POST',
			body: JSON.stringify(params)
		}),
	wizardServiceAction: (id, action) =>
		request(`/servers/${id}/wizard/service/${action}`, { method: 'POST' }),
	serverVersion: (id) => request(`/servers/${id}/version`),
	forceServerUpdate: (id) =>
		request(`/servers/${id}/force-update`, { method: 'POST' }),
	restartServer: (id) =>
		request(`/servers/${id}/restart`, { method: 'POST' }),
	setServerUpdateChannel: (id, channel) =>
		request(`/servers/${id}/update-channel`, { method: 'PUT', body: JSON.stringify({ channel }) }),
	setServerUpdateInterval: (id, interval_secs) =>
		request(`/servers/${id}/update-interval`, { method: 'PUT', body: JSON.stringify({ interval_secs }) }),
	setServerUpdateEnabled: (id, enabled) =>
		request(`/servers/${id}/update-enabled`, { method: 'PUT', body: JSON.stringify({ enabled }) }),
	checkServerGameLog: (id, path) =>
		request(`/servers/${id}/check-game-log`, { method: 'POST', body: JSON.stringify({ path }) }),
	checkGameLog: (path) =>
		request(`/config/check-game-log`, { method: 'POST', body: JSON.stringify({ path }) }),

	// Phase 3 — per-server live control (standalone parity)
	serverLive: (id) => request(`/servers/${id}/live`),
	serverPlayers: (id) => request(`/servers/${id}/players`),
	serverPlayerMute: (id, cid) =>
		request(`/servers/${id}/players/${cid}/mute`, { method: 'POST' }),
	serverPlayerUnmute: (id, cid) =>
		request(`/servers/${id}/players/${cid}/unmute`, { method: 'POST' }),
	serverMaps: (id) => request(`/servers/${id}/maps`),
	serverRefreshMaps: (id) => request(`/servers/${id}/maps/refresh`, { method: 'POST' }),
	serverChangeMap: (id, map) =>
		request(`/servers/${id}/map`, { method: 'POST', body: JSON.stringify({ map }) }),
	serverGetMapcycle: (id) => request(`/servers/${id}/mapcycle`),
	serverSetMapcycle: (id, maps) =>
		request(`/servers/${id}/mapcycle`, { method: 'PUT', body: JSON.stringify({ maps }) }),
	serverGetServerCfg: (id) => request(`/servers/${id}/server-cfg`),
	serverSaveServerCfg: (id, path, contents) =>
		request(`/servers/${id}/server-cfg`, { method: 'PUT', body: JSON.stringify({ path, contents }) }),
	serverGetCvar: (id, name) => request(`/servers/${id}/cvar/${encodeURIComponent(name)}`),
	serverSetCvar: (id, name, value) =>
		request(`/servers/${id}/cvar/${encodeURIComponent(name)}`, { method: 'PUT', body: JSON.stringify({ value }) }),
	serverListMapConfigs: (id) => request(`/servers/${id}/map-configs`),
	serverSaveMapConfig: (id, config) =>
		request(`/servers/${id}/map-configs`, { method: 'POST', body: JSON.stringify(config) }),
	serverDeleteMapConfig: (id, mapConfigId) =>
		request(`/servers/${id}/map-configs/${mapConfigId}`, { method: 'DELETE' }),
	serverEnsureMapConfig: (id, mapName) =>
		request(`/servers/${id}/map-configs/by-name/${encodeURIComponent(mapName)}`),
	serverApplyMapConfig: (id, mapName) =>
		request(`/servers/${id}/map-configs/by-name/${encodeURIComponent(mapName)}/apply`, { method: 'POST' }),
	serverResetMapConfig: (id, mapName) =>
		request(`/servers/${id}/map-configs/by-name/${encodeURIComponent(mapName)}/reset`, { method: 'POST' }),
	mapConfigDefaults: () => request('/map-config-defaults'),
	mapConfigDefault: (mapName) => request(`/map-config-defaults/${encodeURIComponent(mapName)}`),
	saveMapConfigDefault: (mapName, def) =>
		request(`/map-config-defaults/${encodeURIComponent(mapName)}`, { method: 'PUT', body: JSON.stringify(def) }),
	deleteMapConfigDefault: (mapName) =>
		request(`/map-config-defaults/${encodeURIComponent(mapName)}`, { method: 'DELETE' }),
	propagateMapConfigDefault: (mapName, overwriteUserEdits = false) =>
		request(`/map-config-defaults/${encodeURIComponent(mapName)}/propagate`, { method: 'POST', body: JSON.stringify({ overwrite_user_edits: overwriteUserEdits }) }),
	serverPenalties: (id, limit = 100, offset = 0) =>
		request(`/servers/${id}/penalties?limit=${limit}&offset=${offset}`),
	serverChat: (id, limit = 100, beforeId = null) => {
		const q = beforeId ? `?limit=${limit}&before_id=${beforeId}` : `?limit=${limit}`;
		return request(`/servers/${id}/chat${q}`);
	},
	serverAuditLog: (id, limit = 100, offset = 0) =>
		request(`/servers/${id}/audit-log?limit=${limit}&offset=${offset}`),
	serverListPlugins: (id) => request(`/servers/${id}/plugins`),
	serverUpdatePlugin: (id, name, body) =>
		request(`/servers/${id}/plugins/${encodeURIComponent(name)}`, { method: 'PUT', body: JSON.stringify(body) }),

	// Map repository (external .pk3 browser, master-side cache)
	mapRepoSearch: (q = '', limit = 50, offset = 0) => {
		const p = new URLSearchParams();
		if (q) p.set('q', q);
		p.set('limit', String(limit));
		p.set('offset', String(offset));
		return request(`/map-repo?${p.toString()}`);
	},
	mapRepoStatus: () => request('/map-repo/status'),
	mapRepoRefresh: () => request('/map-repo/refresh', { method: 'POST' }),
	serverImportMap: (id, filename) =>
		request(`/servers/${id}/maps/import`, { method: 'POST', body: JSON.stringify({ filename }) }),
	serverMissingMaps: (id, maps) =>
		request(`/servers/${id}/maps/missing`, { method: 'POST', body: JSON.stringify({ maps }) }),
	// Standalone-mode equivalents
	localImportMap: (filename) =>
		request('/server/maps/import', { method: 'POST', body: JSON.stringify({ filename }) }),
	localMissingMaps: (maps) =>
		request('/server/maps/missing', { method: 'POST', body: JSON.stringify({ maps }) }),

	// Pairing (master mode)
	enablePairing: (expiry_minutes = 30) =>
		request('/pairing/enable', { method: 'POST', body: JSON.stringify({ expiry_minutes }) }),
	disablePairing: () =>
		request('/pairing/disable', { method: 'POST' }),

	// Hubs (master mode)
	hubs: () => request('/hubs').then(r => r.hubs ?? r),
	hub: (id) => request(`/hubs/${id}`),
	deleteHub: (id) => request(`/hubs/${id}`, { method: 'DELETE' }),
	hubMetrics: (id, range = '1h') => request(`/hubs/${id}/metrics?range=${encodeURIComponent(range)}`),
	hubInstallClient: (id, body) =>
		request(`/hubs/${id}/clients`, { method: 'POST', body: JSON.stringify(body) }),
	hubActionProgress: (id, actionId) =>
		request(`/hubs/${id}/actions/${encodeURIComponent(actionId)}`),
	hubUninstallClient: (id, slug, remove_data = false) =>
		request(`/hubs/${id}/clients/${encodeURIComponent(slug)}?remove_data=${remove_data}`, { method: 'DELETE' }),
	hubClientAction: (id, slug, action) =>
		request(`/hubs/${id}/clients/${encodeURIComponent(slug)}/action`, { method: 'POST', body: JSON.stringify({ action }) }),
	hubInstallGameServer: (id, body) =>
		request(`/hubs/${id}/game-server`, { method: 'POST', body: JSON.stringify(body) }),
	hubReconfigureGameServer: (id, slug, body) =>
		request(`/hubs/${id}/clients/${encodeURIComponent(slug)}/reconfigure-game-server`, {
			method: 'POST',
			body: JSON.stringify(body),
		}),
	hubRestart: (id) => request(`/hubs/${id}/restart`, { method: 'POST' }),
	hubVersion: (id) => request(`/hubs/${id}/version`),
	forceHubUpdate: (id) => request(`/hubs/${id}/force-update`, { method: 'POST' }),
	setHubUpdateChannel: (id, channel) =>
		request(`/hubs/${id}/update-channel`, { method: 'PUT', body: JSON.stringify({ channel }) }),
	setHubUpdateInterval: (id, interval_secs) =>
		request(`/hubs/${id}/update-interval`, { method: 'PUT', body: JSON.stringify({ interval_secs }) }),
	setHubUpdateEnabled: (id, enabled) =>
		request(`/hubs/${id}/update-enabled`, { method: 'PUT', body: JSON.stringify({ enabled }) }),
};
