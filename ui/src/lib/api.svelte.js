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

	// Players
	players: () => request('/players').then(r => r.players),
	player: (id) => request(`/players/${id}`),
	kickPlayer: (cid, reason) =>
		request(`/players/${cid}/kick`, { method: 'POST', body: JSON.stringify({ reason }) }),
	banPlayer: (cid, reason, duration) =>
		request(`/players/${cid}/ban`, { method: 'POST', body: JSON.stringify({ reason, duration }) }),
	messagePlayer: (cid, message) =>
		request(`/players/${cid}/message`, { method: 'POST', body: JSON.stringify({ message }) }),
	searchClients: (query) => request(`/clients/search?q=${encodeURIComponent(query)}`).then(r => r.clients),
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

	// Plugins
	plugins: () => request('/plugins'),

	// Stats
	leaderboard: (limit = 25, offset = 0) =>
		request(`/stats/leaderboard?limit=${limit}&offset=${offset}`),
	playerStats: (id) => request(`/stats/player/${id}`),
	weaponStats: () => request('/stats/weapons'),
	mapStats: () => request('/stats/maps'),
	dashboardSummary: () => request('/stats/summary'),

	// Chat
	chat: (limit = 50, beforeId = null) =>
		request(`/chat?limit=${limit}${beforeId ? '&before_id=' + beforeId : ''}`).then(r => r.messages),

	// Votes
	votes: (limit = 20) =>
		request(`/votes?limit=${limit}`).then(r => r.votes),

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
		request('/users/me/password', { method: 'PUT', body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }) })
};
