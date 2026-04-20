<script>
	import { api, setToken } from '$lib/api.svelte.js';
	import { onMount } from 'svelte';

	let step = $state(1);
	let loading = $state(true);
	let submitting = $state(false);
	let error = $state('');
	let success = $state(false);

	// Setup status from API
	let status = $state(null);

	// Form fields
	let adminUsername = $state('admin');
	let adminPassword = $state('');
	let adminPasswordConfirm = $state('');
	let botName = $state('R3');
	let serverIp = $state('');
	let serverPort = $state(27960);
	let rconPassword = $state('');
	let gameLog = $state('');

	onMount(async () => {
		try {
			status = await api.setupStatus();
			if (!status.needs_setup) {
				window.location.href = '/login';
				return;
			}
		} catch (e) {
			error = 'Could not connect to the R3 API.';
		} finally {
			loading = false;
		}
	});

	function nextStep() {
		error = '';
		if (step === 1) {
			if (!adminUsername.trim()) { error = 'Username is required'; return; }
			if (adminPassword.length < 6) { error = 'Password must be at least 6 characters'; return; }
			if (adminPassword !== adminPasswordConfirm) { error = 'Passwords do not match'; return; }
		}
		step++;
	}

	function prevStep() {
		error = '';
		step--;
	}

	async function handleSubmit() {
		error = '';
		submitting = true;
		try {
			const data = {
				admin_username: adminUsername.trim(),
				admin_password: adminPassword,
			};
			if (botName.trim()) data.bot_name = botName.trim();
			if (status?.mode !== 'client') {
				if (serverIp.trim()) data.server_ip = serverIp.trim();
				if (serverPort) data.server_port = serverPort;
				if (rconPassword) data.rcon_password = rconPassword;
				if (gameLog.trim()) data.game_log = gameLog.trim();
			}
			await api.completeSetup(data);
			success = true;
			// Auto-login after setup
			setTimeout(async () => {
				try {
					const res = await api.login(adminUsername.trim(), adminPassword);
					setToken(res.token);
					window.location.href = '/';
				} catch {
					window.location.href = '/login';
				}
			}, 2000);
		} catch (e) {
			error = e.message || 'Setup failed';
		} finally {
			submitting = false;
		}
	}

	let isMaster = $derived(status?.mode === 'master');
	let isClient = $derived(status?.mode === 'client');
	let totalSteps = $derived(isClient ? 2 : 3);
</script>

<svelte:head>
	<title>R3 — Setup Wizard</title>
</svelte:head>

<div class="flex min-h-screen items-center justify-center px-4">
	<div class="w-full max-w-lg animate-fade-in">
		<div class="mb-8 text-center">
			<div class="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-2xl bg-accent/10 ring-1 ring-accent/20">
				<span class="text-2xl font-bold text-accent">R3</span>
			</div>
			<h1 class="text-2xl font-semibold text-surface-100">Setup Wizard</h1>
			<p class="mt-1 text-sm text-surface-500">
				{#if loading}
					Connecting...
				{:else if status}
					{status.mode.charAt(0).toUpperCase() + status.mode.slice(1)} mode — v{status.version}
				{/if}
			</p>
		</div>

		{#if loading}
			<div class="card p-8 text-center">
				<span class="inline-block h-6 w-6 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></span>
			</div>
		{:else if success}
			<div class="card p-8 text-center space-y-4">
				<div class="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-green-500/10 ring-1 ring-green-500/30">
					<svg class="h-6 w-6 text-green-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
						<path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
					</svg>
				</div>
				<h2 class="text-lg font-semibold text-surface-100">Setup Complete!</h2>
				<p class="text-sm text-surface-400">Redirecting to the dashboard...</p>
			</div>
		{:else}
			<!-- Progress bar -->
			<div class="mb-6 flex items-center gap-2">
				{#each Array(totalSteps) as _, i}
					<div class="h-1 flex-1 rounded-full transition-colors {i < step ? 'bg-accent' : 'bg-surface-700'}"></div>
				{/each}
				<span class="text-xs text-surface-500 ml-2">{step}/{totalSteps}</span>
			</div>

			<form onsubmit={(e) => { e.preventDefault(); step === totalSteps ? handleSubmit() : nextStep(); }} class="card p-6 space-y-5">
				{#if error}
					<div class="rounded-lg bg-red-500/10 px-4 py-3 text-sm text-red-400 ring-1 ring-red-500/20">
						{error}
					</div>
				{/if}

				<!-- Step 1: Admin Account -->
				{#if step === 1}
					<div>
						<h2 class="text-lg font-semibold text-surface-100 mb-1">Create Admin Account</h2>
						<p class="text-sm text-surface-500 mb-4">This will be the main administrator for the dashboard.</p>
					</div>

					<div>
						<label for="username" class="mb-1.5 block text-sm font-medium text-surface-300">Username</label>
						<input id="username" type="text" bind:value={adminUsername} class="input" placeholder="admin" autocomplete="username" required />
					</div>

					<div>
						<label for="password" class="mb-1.5 block text-sm font-medium text-surface-300">Password</label>
						<input id="password" type="password" bind:value={adminPassword} class="input" placeholder="••••••••" autocomplete="new-password" required />
					</div>

					<div>
						<label for="password2" class="mb-1.5 block text-sm font-medium text-surface-300">Confirm Password</label>
						<input id="password2" type="password" bind:value={adminPasswordConfirm} class="input" placeholder="••••••••" autocomplete="new-password" required />
					</div>

				<!-- Step 2: Server Settings (skip for client mode) -->
				{:else if step === 2 && !isClient}
					<div>
						<h2 class="text-lg font-semibold text-surface-100 mb-1">Game Server</h2>
						<p class="text-sm text-surface-500 mb-4">
							{#if isMaster}
								Configure the game server this master bot manages. Clients can be added later.
							{:else}
								Configure the Urban Terror game server connection.
							{/if}
						</p>
					</div>

					<div>
						<label for="botname" class="mb-1.5 block text-sm font-medium text-surface-300">Bot Name</label>
						<input id="botname" type="text" bind:value={botName} class="input" placeholder="R3" />
						<p class="mt-1 text-xs text-surface-600">Name shown in-game when the bot sends messages.</p>
					</div>

					<div class="grid grid-cols-3 gap-3">
						<div class="col-span-2">
							<label for="sip" class="mb-1.5 block text-sm font-medium text-surface-300">Server IP</label>
							<input id="sip" type="text" bind:value={serverIp} class="input" placeholder="127.0.0.1" />
						</div>
						<div>
							<label for="sport" class="mb-1.5 block text-sm font-medium text-surface-300">Port</label>
							<input id="sport" type="number" bind:value={serverPort} class="input" placeholder="27960" />
						</div>
					</div>

					<div>
						<label for="rcon" class="mb-1.5 block text-sm font-medium text-surface-300">RCON Password</label>
						<input id="rcon" type="password" bind:value={rconPassword} class="input" placeholder="rcon password" autocomplete="off" />
					</div>

					<div>
						<label for="glog" class="mb-1.5 block text-sm font-medium text-surface-300">Game Log Path</label>
						<input id="glog" type="text" bind:value={gameLog} class="input" placeholder="/home/user/.q3a/q3ut4/games.log" />
						<p class="mt-1 text-xs text-surface-600">Absolute path to the UrT games.log file.</p>
					</div>

				<!-- Final step: Review -->
				{:else}
					<div>
						<h2 class="text-lg font-semibold text-surface-100 mb-1">Review & Finish</h2>
						<p class="text-sm text-surface-500 mb-4">Confirm your settings and complete the setup.</p>
					</div>

					<div class="space-y-3 text-sm">
						<div class="flex justify-between py-2 border-b border-surface-700">
							<span class="text-surface-400">Mode</span>
							<span class="text-surface-200 font-medium">{status?.mode}</span>
						</div>
						<div class="flex justify-between py-2 border-b border-surface-700">
							<span class="text-surface-400">Admin User</span>
							<span class="text-surface-200 font-medium">{adminUsername}</span>
						</div>
						{#if !isClient}
							{#if botName}
								<div class="flex justify-between py-2 border-b border-surface-700">
									<span class="text-surface-400">Bot Name</span>
									<span class="text-surface-200 font-medium">{botName}</span>
								</div>
							{/if}
							{#if serverIp}
								<div class="flex justify-between py-2 border-b border-surface-700">
									<span class="text-surface-400">Game Server</span>
									<span class="text-surface-200 font-medium">{serverIp}:{serverPort}</span>
								</div>
							{/if}
							{#if rconPassword}
								<div class="flex justify-between py-2 border-b border-surface-700">
									<span class="text-surface-400">RCON</span>
									<span class="text-surface-200 font-medium">configured</span>
								</div>
							{/if}
							{#if gameLog}
								<div class="flex justify-between py-2 border-b border-surface-700">
									<span class="text-surface-400">Game Log</span>
									<span class="text-surface-200 font-mono text-xs">{gameLog}</span>
								</div>
							{/if}
						{/if}
					</div>
				{/if}

				<!-- Nav buttons -->
				<div class="flex justify-between pt-2">
					{#if step > 1}
						<button type="button" onclick={prevStep} class="btn-ghost">Back</button>
					{:else}
						<div></div>
					{/if}

					<button type="submit" class="btn-primary" disabled={submitting}>
						{#if submitting}
							<span class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-white/20 border-t-white mr-2"></span>
						{/if}
						{step === totalSteps ? 'Complete Setup' : 'Next'}
					</button>
				</div>
			</form>
		{/if}

		<p class="mt-6 text-center text-xs text-surface-600">
			Rusty Rules Referee — Game Server Administration
		</p>
	</div>
</div>
