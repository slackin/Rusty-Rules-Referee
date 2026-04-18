<script>
	import { api, setToken } from '$lib/api.js';

	let username = $state('');
	let password = $state('');
	let error = $state('');
	let loading = $state(false);

	async function handleLogin(e) {
		e.preventDefault();
		error = '';
		loading = true;
		try {
			const res = await api.login(username, password);
			setToken(res.token);
			window.location.href = '/';
		} catch (err) {
			error = err.message || 'Login failed';
		} finally {
			loading = false;
		}
	}
</script>

<svelte:head>
	<title>R3 Admin — Login</title>
</svelte:head>

<div class="flex min-h-screen items-center justify-center px-4">
	<div class="w-full max-w-sm animate-fade-in">
		<div class="mb-8 text-center">
			<div class="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-2xl bg-accent/10 ring-1 ring-accent/20">
				<span class="text-2xl font-bold text-accent">R3</span>
			</div>
			<h1 class="text-2xl font-semibold text-surface-100">Welcome back</h1>
			<p class="mt-1 text-sm text-surface-500">Sign in to the admin dashboard</p>
		</div>

		<form onsubmit={handleLogin} class="card p-6 space-y-4">
			{#if error}
				<div class="rounded-lg bg-red-500/10 px-4 py-3 text-sm text-red-400 ring-1 ring-red-500/20">
					{error}
				</div>
			{/if}

			<div>
				<label for="username" class="mb-1.5 block text-sm font-medium text-surface-300">Username</label>
				<input
					id="username"
					type="text"
					bind:value={username}
					class="input"
					placeholder="admin"
					autocomplete="username"
					required
				/>
			</div>

			<div>
				<label for="password" class="mb-1.5 block text-sm font-medium text-surface-300">Password</label>
				<input
					id="password"
					type="password"
					bind:value={password}
					class="input"
					placeholder="••••••••"
					autocomplete="current-password"
					required
				/>
			</div>

			<button type="submit" class="btn-primary w-full" disabled={loading}>
				{#if loading}
					<span class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-white/20 border-t-white"></span>
				{/if}
				Sign in
			</button>
		</form>

		<p class="mt-6 text-center text-xs text-surface-600">
			Rusty Rules Referee — Game Server Administration
		</p>
	</div>
</div>
