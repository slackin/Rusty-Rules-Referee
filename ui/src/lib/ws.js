import { getToken } from './api.js';

/** @typedef {(event: any) => void} EventHandler */

/** @type {WebSocket|null} */
let ws = null;
/** @type {EventHandler[]} */
let listeners = [];
let reconnectTimer = null;

export function connectWs() {
	const token = getToken();
	if (!token) return;

	const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
	ws = new WebSocket(`${proto}//${location.host}/ws?token=${token}`);

	ws.onmessage = (msg) => {
		try {
			const event = JSON.parse(msg.data);
			listeners.forEach((fn) => fn(event));
		} catch {
			// ignore non-JSON messages
		}
	};

	ws.onclose = () => {
		ws = null;
		// Reconnect after 3s
		if (reconnectTimer) clearTimeout(reconnectTimer);
		reconnectTimer = setTimeout(connectWs, 3000);
	};

	ws.onerror = () => {
		ws?.close();
	};
}

export function disconnectWs() {
	if (reconnectTimer) clearTimeout(reconnectTimer);
	reconnectTimer = null;
	ws?.close();
	ws = null;
}

/** @param {EventHandler} fn */
export function onEvent(fn) {
	listeners.push(fn);
	return () => {
		listeners = listeners.filter((l) => l !== fn);
	};
}
