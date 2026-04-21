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

	// Config scanner state
	let scanning = $state(false);
	let scannedFiles = $state([]);
	let showScanned = $state(false);

	// File browser state
	let browsing = $state(false);
	let browseEntries = $state([]);
	let browsePath = $state('');
	let browseParent = $state(null);
	let browseHome = $state('');
	let browseLoading = $state(false);

	// Config analysis state
	let analyzing = $state(false);
	let analysisResult = $state(null);
	let selectedCfgPath = $state('');

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
				if (selectedCfgPath.trim()) data.server_cfg_path = selectedCfgPath.trim();
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

	// Scan for config files in known UrT directories
	async function scanForConfigs() {
		scanning = true;
		error = '';
		try {
			const res = await api.setupScanConfigs();
			scannedFiles = res.files || [];
			showScanned = true;
			if (scannedFiles.length === 0) {
				error = 'No config files found in common UrT directories. Use Browse to locate your server.cfg manually.';
			}
		} catch (e) {
			error = e.message || 'Failed to scan for config files';
		} finally {
			scanning = false;
		}
	}

	// Open the file browser
	async function openBrowser(path) {
		browseLoading = true;
		error = '';
		try {
			const res = await api.setupBrowse(path || '');
			browseEntries = res.entries || [];
			browsePath = res.path;
			browseParent = res.parent || null;
			browseHome = res.home || '';
			browsing = true;
		} catch (e) {
			error = e.message || 'Failed to browse directory';
		} finally {
			browseLoading = false;
		}
	}

	// Navigate into a directory
	async function browseNavigate(dirName) {
		const newPath = browsePath.endsWith('/') ? browsePath + dirName : browsePath + '/' + dirName;
		await openBrowser(newPath);
	}

	// Navigate up to parent
	async function browseUp() {
		if (browseParent) {
			await openBrowser(browseParent);
		}
	}

	// Select a .cfg file from the browser
	function browseSelectFile(fileName) {
		const fullPath = browsePath.endsWith('/') ? browsePath + fileName : browsePath + '/' + fileName;
		selectConfigFile(fullPath);
		browsing = false;
	}

	// Close the file browser
	function closeBrowser() {
		browsing = false;
	}

	// Select a config file (from scan results or browser) and analyze it
	async function selectConfigFile(path) {
		selectedCfgPath = path;
		analyzing = true;
		error = '';
		analysisResult = null;
		try {
			const res = await api.setupAnalyzeCfg(path);
			analysisResult = res;
			// Auto-fill form fields from parsed settings
			if (res.settings) {
				if (res.settings.rcon_password) rconPassword = res.settings.rcon_password;
				if (res.settings.port) serverPort = res.settings.port;
				if (res.settings.game_log) gameLog = res.settings.game_log;
			}
		} catch (e) {
			error = e.message || 'Failed to analyze config file';
		} finally {
			analyzing = false;
		}
	}

	function checkIcon(status) {
		if (status === 'ok') return '✓';
		if (status === 'error') return '✗';
		if (status === 'warning') return '!';
		return 'i';
	}

	function checkColor(status) {
		if (status === 'ok') return 'text-green-400';
		if (status === 'error') return 'text-red-400';
		if (status === 'warning') return 'text-yellow-400';
		return 'text-blue-400';
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
								Locate your server.cfg to auto-detect settings, or enter them manually.
							{/if}
						</p>
					</div>

					<!-- Config file detection -->
					<div class="rounded-lg bg-surface-800/50 p-4 ring-1 ring-surface-700 space-y-3">
						<div class="flex items-center justify-between">
							<span class="text-sm font-medium text-surface-300">Server Config File</span>
							<div class="flex gap-2">
								<button type="button" onclick={scanForConfigs} disabled={scanning} class="btn-ghost text-xs">
									{#if scanning}
										<span class="inline-block h-3 w-3 animate-spin rounded-full border-2 border-accent/20 border-t-accent mr-1"></span>
									{/if}
									Scan
								</button>
								<button type="button" onclick={() => openBrowser('')} disabled={browseLoading} class="btn-ghost text-xs">
									Browse
								</button>
							</div>
						</div>

						{#if selectedCfgPath}
							<div class="flex items-center gap-2 rounded bg-surface-700/50 px-3 py-2 text-xs">
								<svg class="h-4 w-4 text-accent shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
									<path stroke-linecap="round" stroke-linejoin="round" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
								</svg>
								<span class="text-surface-300 font-mono truncate">{selectedCfgPath}</span>
								{#if analyzing}
									<span class="inline-block h-3 w-3 animate-spin rounded-full border-2 border-accent/20 border-t-accent ml-auto shrink-0"></span>
								{/if}
							</div>
						{/if}

						<!-- Scan results -->
						{#if showScanned && scannedFiles.length > 0}
							<div class="space-y-1">
								<span class="text-xs text-surface-500">Found {scannedFiles.length} config file{scannedFiles.length !== 1 ? 's' : ''}:</span>
								<div class="max-h-32 overflow-y-auto space-y-1">
									{#each scannedFiles as file}
										<button
											type="button"
											onclick={() => { showScanned = false; selectConfigFile(file.path); }}
											class="w-full text-left rounded px-3 py-1.5 text-xs font-mono text-surface-300 hover:bg-surface-700 transition-colors truncate"
											title={file.path}
										>
											{file.path}
										</button>
									{/each}
								</div>
							</div>
						{/if}

						<!-- File browser modal -->
						{#if browsing}
							<div class="space-y-2">
								<div class="flex items-center gap-2">
									{#if browseParent}
										<button type="button" onclick={browseUp} class="btn-ghost text-xs px-2" title="Go up">
											<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
												<path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7" />
											</svg>
										</button>
									{/if}
									<span class="text-xs text-surface-400 font-mono truncate flex-1" title={browsePath}>{browsePath}</span>
									<button type="button" onclick={closeBrowser} class="btn-ghost text-xs px-2" title="Close">
										<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
											<path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
										</svg>
									</button>
								</div>
								<div class="max-h-48 overflow-y-auto rounded bg-surface-900/50 ring-1 ring-surface-700">
									{#if browseLoading}
										<div class="p-4 text-center">
											<span class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></span>
										</div>
									{:else if browseEntries.length === 0}
										<div class="p-3 text-xs text-surface-500 text-center">No .cfg files or directories found here.</div>
									{:else}
										{#each browseEntries as entry}
											{#if entry.is_dir}
												<button
													type="button"
													onclick={() => browseNavigate(entry.name)}
													class="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-surface-300 hover:bg-surface-700/50 transition-colors"
												>
													<svg class="h-4 w-4 text-accent shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
														<path stroke-linecap="round" stroke-linejoin="round" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
													</svg>
													<span class="truncate">{entry.name}</span>
												</button>
											{:else}
												<button
													type="button"
													onclick={() => browseSelectFile(entry.name)}
													class="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-surface-300 hover:bg-surface-700/50 transition-colors"
												>
													<svg class="h-4 w-4 text-surface-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
														<path stroke-linecap="round" stroke-linejoin="round" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
													</svg>
													<span class="font-mono truncate">{entry.name}</span>
													<span class="ml-auto text-surface-600">{(entry.size / 1024).toFixed(1)}KB</span>
												</button>
											{/if}
										{/each}
									{/if}
								</div>
							</div>
						{/if}

						<!-- Health checks from analysis -->
						{#if analysisResult?.checks?.length}
							<div class="space-y-1 pt-1">
								<span class="text-xs text-surface-500">Compatibility checks:</span>
								{#each analysisResult.checks as check}
									<div class="flex items-start gap-2 text-xs">
										<span class="shrink-0 font-bold {checkColor(check.status)}">{checkIcon(check.status)}</span>
										<span class="text-surface-400">{check.message}</span>
									</div>
								{/each}
							</div>
						{/if}
					</div>

					<!-- Server settings (auto-filled or manual) -->
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
						{#if analysisResult?.settings?.rcon_password}
							<p class="mt-1 text-xs text-green-500">Auto-detected from config file</p>
						{/if}
					</div>

					<div>
						<label for="glog" class="mb-1.5 block text-sm font-medium text-surface-300">Game Log Path</label>
						<input id="glog" type="text" bind:value={gameLog} class="input" placeholder="/home/user/.q3a/q3ut4/games.log" />
						{#if analysisResult?.settings?.game_log}
							<p class="mt-1 text-xs text-green-500">Auto-detected from config file</p>
						{:else}
							<p class="mt-1 text-xs text-surface-600">Absolute path to the UrT games.log file.</p>
						{/if}
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
							{#if selectedCfgPath}
								<div class="flex justify-between py-2 border-b border-surface-700">
									<span class="text-surface-400">Config File</span>
									<span class="text-surface-200 font-mono text-xs truncate max-w-[200px]" title={selectedCfgPath}>{selectedCfgPath.split('/').pop()}</span>
								</div>
							{/if}
							{#if serverIp}
								<div class="flex justify-between py-2 border-b border-surface-700">
									<span class="text-surface-400">Game Server</span>
									<span class="text-surface-200 font-medium">{serverIp}:{serverPort}</span>
								</div>
							{:else if serverPort !== 27960}
								<div class="flex justify-between py-2 border-b border-surface-700">
									<span class="text-surface-400">Port</span>
									<span class="text-surface-200 font-medium">{serverPort}</span>
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
									<span class="text-surface-200 font-mono text-xs truncate max-w-[250px]" title={gameLog}>{gameLog}</span>
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
