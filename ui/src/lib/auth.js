import { api, getToken, setToken } from './api.js';

/** @type {{ user: any, loading: boolean }} */
let auth = $state({ user: null, loading: true });

export function getAuth() {
	return auth;
}

export async function checkAuth() {
	const token = getToken();
	if (!token) {
		auth.user = null;
		auth.loading = false;
		return false;
	}
	try {
		auth.user = await api.me();
		auth.loading = false;
		return true;
	} catch {
		setToken(null);
		auth.user = null;
		auth.loading = false;
		return false;
	}
}

export function logout() {
	setToken(null);
	auth.user = null;
	window.location.href = '/login';
}
