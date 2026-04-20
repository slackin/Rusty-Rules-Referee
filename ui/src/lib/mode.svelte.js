/**
 * Mode store — tracks whether the app is in standalone, master, or client mode.
 * Fetched once from /api/v1/setup/status on app load.
 */

let mode = $state('standalone');
let modeLoaded = $state(false);

export function getMode() { return mode; }
export function isMaster() { return mode === 'master'; }
export function isClient() { return mode === 'client'; }
export function isModeLoaded() { return modeLoaded; }

export async function fetchMode() {
	try {
		const res = await fetch('/api/v1/setup/status');
		if (res.ok) {
			const data = await res.json();
			mode = data.mode || 'standalone';
		}
	} catch {
		// Default to standalone on error
	}
	modeLoaded = true;
}
