/**
 * Reactive live data store — maintains server status and online players
 * via WebSocket events + periodic polling fallback.
 *
 * All pages import from this single source of truth instead of fetching independently.
 */
import { api } from './api.svelte.js';
import { onEvent } from './ws.js';

// ---- Reactive state ----
let serverStatus = $state(null);
let onlinePlayers = $state([]);
let recentEvents = $state([]);
let recentChat = $state([]);
let recentVotes = $state([]);
let initialized = $state(false);
let refreshing = $state(false);

// ---- Polling interval ----
let pollTimer = null;
let unsubWs = null;
const POLL_INTERVAL = 15_000; // 15s fallback poll

// ---- Exports ----

export function getServerStatus() { return serverStatus; }
export function getOnlinePlayers() { return onlinePlayers; }
export function getRecentEvents() { return recentEvents; }
export function getRecentChat() { return recentChat; }
export function getRecentVotes() { return recentVotes; }
export function isInitialized() { return initialized; }
export function isRefreshing() { return refreshing; }

/**
 * Initialize the live store: fetch initial data, subscribe to WS events, start polling.
 * Call once from the app layout. Returns a cleanup function.
 */
export function initLiveStore() {
	// Fetch initial data
	refresh();

	// Subscribe to WebSocket events
	unsubWs = onEvent(handleEvent);

	// Start fallback polling
	pollTimer = setInterval(refresh, POLL_INTERVAL);

	return () => {
		if (unsubWs) { unsubWs(); unsubWs = null; }
		if (pollTimer) { clearInterval(pollTimer); pollTimer = null; }
	};
}

/**
 * Force a full refresh of server status + players from the REST API.
 */
export async function refresh() {
	if (refreshing) return;
	refreshing = true;
	try {
		const [s, p] = await Promise.all([api.serverStatus(), api.players()]);
		serverStatus = s;
		onlinePlayers = p || [];
		initialized = true;
		// Also fetch chat + votes (non-blocking)
		api.chat(50).then(msgs => { recentChat = msgs || []; }).catch(() => {});
		api.votes(20).then(v => { recentVotes = v || []; }).catch(() => {});
	} catch (e) {
		console.error('[live] refresh failed:', e);
	}
	refreshing = false;
}

/**
 * Handle an incoming WebSocket event — update live state reactively.
 */
function handleEvent(evt) {
	const type = evt.type;

	// Always add to recent events feed (max 50)
	recentEvents = [evt, ...recentEvents.slice(0, 49)];

	// Push say/team-say events to recentChat
	if (type === 'EVT_CLIENT_SAY' || type === 'EVT_CLIENT_TEAM_SAY') {
		const chatMsg = {
			id: Date.now(),
			client_id: evt.client_id,
			client_name: evt.client_name || 'Unknown',
			channel: type === 'EVT_CLIENT_TEAM_SAY' ? 'team' : 'all',
			message: evt.data?.text || '',
			time_add: new Date().toISOString(),
		};
		recentChat = [chatMsg, ...recentChat.slice(0, 99)];
	}

	// Push callvote events to recentVotes
	if (type === 'EVT_CLIENT_CALLVOTE') {
		const voteMsg = {
			id: Date.now(),
			client_id: evt.client_id,
			client_name: evt.client_name || 'Unknown',
			vote_type: (evt.data?.text || '').split(' ')[0] || 'unknown',
			vote_data: evt.data?.text || '',
			time_add: new Date().toISOString(),
		};
		recentVotes = [voteMsg, ...recentVotes.slice(0, 49)];
	}

	switch (type) {
		// Player connected — add to list (will be fully populated on next poll)
		case 'EVT_CLIENT_AUTH':
		case 'EVT_CLIENT_CONNECT': {
			// Trigger a fast refresh to get full player data
			scheduleQuickRefresh();
			break;
		}

		// Player disconnected — remove from list immediately
		case 'EVT_CLIENT_DISCONNECT': {
			if (evt.client_id != null) {
				onlinePlayers = onlinePlayers.filter(p => p.id !== evt.client_id);
			}
			break;
		}

		// Team change — update player team
		case 'EVT_CLIENT_TEAM_CHANGE':
		case 'EVT_CLIENT_TEAM_CHANGE2':
		case 'EVT_CLIENT_JOIN': {
			scheduleQuickRefresh();
			break;
		}

		// Map change — refresh everything
		case 'EVT_GAME_MAP_CHANGE':
		case 'EVT_GAME_EXIT': {
			scheduleQuickRefresh();
			break;
		}

		// Name change — update player name
		case 'EVT_CLIENT_NAME_CHANGE': {
			if (evt.client_id != null && evt.data?.text) {
				onlinePlayers = onlinePlayers.map(p =>
					p.id === evt.client_id ? { ...p, name: evt.data.text } : p
				);
			}
			break;
		}

		// Round events — refresh status for score updates
		case 'EVT_GAME_ROUND_START':
		case 'EVT_GAME_ROUND_END': {
			scheduleQuickRefresh();
			break;
		}
	}
}

// ---- Quick refresh debounce ----
let quickRefreshTimer = null;

function scheduleQuickRefresh() {
	if (quickRefreshTimer) return; // already scheduled
	quickRefreshTimer = setTimeout(() => {
		quickRefreshTimer = null;
		refresh();
	}, 500); // 500ms debounce — batches rapid events
}
