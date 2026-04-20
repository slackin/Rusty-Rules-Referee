<script>
	import { api } from '$lib/api.svelte.js';
	import { page } from '$app/stores';
	import { Server, Wifi, WifiOff, Users, Map, Terminal, MessageSquare, ArrowLeft, UserX, ShieldBan, Send, RefreshCw, Settings, Save, Download, FileSearch, Wrench, FolderOpen, Check, AlertTriangle, Loader2, Folder, FileText, ChevronRight } from 'lucide-svelte';

	let serverId = $derived(Number($page.params.id));
	let server = $state(null);
	let loading = $state(true);
	let error = $state('');

	// Server config
	let configLoading = $state(false);
	let configSaving = $state(false);
	let configResult = $state(null);
	let configAddress = $state('');
	let configPort = $state(27960);
	let configRconPassword = $state('');
	let configGameLog = $state('');

	// Setup wizard state
	let setupMethod = $state(null); // 'install' | 'scan' | 'manual' | null
	let setupStep = $state(0);

	// Install flow
	let installPath = $state('/opt/urbanterror');
	let installStarted = $state(false);
	let installProgress = $state(null);
	let installError = $state('');
	let installPolling = $state(false);

	// Scan flow
	let scanning = $state(false);
	let scanResults = $state(null);
	let scanError = $state('');
	let selectedConfigPath = $state('');
	let parsing = $state(false);
	let parsedConfig = $state(null);
	let parseError = $state('');

	// Browse flow
	let browsing = $state(false);
	let browseEntries = $state(null);
	let browsePath = $state('');
	let browseError = $state('');

	// RCON
	let rconCommand = $state('');
	let rconHistory = $state([]);
	let rconSending = $state(false);

	// Say
	let sayMessage = $state('');
	let saySending = $state(false);
	let sayResult = $state(null);

	// Kick/Ban
	let actionType = $state(null);
	let actionCid = $state('');
	let actionReason = $state('');
	let actionDuration = $state(60);
	let actionSending = $state(false);
	let actionResult = $state(null);

	// Version & update
	let versionInfo = $state(null);
	let versionLoading = $state(false);
	let versionError = $state('');
	let forceUpdating = $state(false);
	let forceUpdateResult = $state(null);

	async function loadServer() {
		try {
			server = await api.server(serverId);
			error = '';
		} catch (e) {
			error = e.message || 'Failed to load server';
		}
		loading = false;
	}

	async function loadConfig() {
		configLoading = true;
		try {
			const res = await api.serverConfig(serverId);
			if (res.config) {
				configAddress = res.config.address || '';
				configPort = res.config.port || 27960;
				configRconPassword = res.config.rcon_password || '';
				configGameLog = res.config.game_log || '';
			}
		} catch (e) {
			// Config may not exist yet — that's fine
		}
		configLoading = false;
	}

	async function saveConfig() {
		configSaving = true;
		configResult = null;
		try {
			const payload = {
				address: configAddress,
				port: Number(configPort),
				rcon_password: configRconPassword,
			};
			if (configGameLog.trim()) payload.game_log = configGameLog;
			const res = await api.updateServerConfig(serverId, payload);
			configResult = { ok: true, message: res.message || 'Configuration saved and pushed' };
			// Reset setup wizard state on successful save
			setupMethod = null;
			setupStep = 0;
			loadServer();
		} catch (e) {
			configResult = { ok: false, message: e.message || 'Failed to save' };
		}
		configSaving = false;
	}

	// --- Install flow ---
	async function startInstall() {
		if (!installPath.trim()) return;
		installStarted = true;
		installError = '';
		installProgress = { stage: 'Starting...', percent: 0 };
		try {
			await api.installGameServer(serverId, installPath);
			pollInstallStatus();
		} catch (e) {
			installError = e.message || 'Failed to start installation';
			installStarted = false;
		}
	}

	async function pollInstallStatus() {
		installPolling = true;
		while (installPolling) {
			try {
				const resp = await api.installStatus(serverId);
				if (resp.InstallProgress) {
					installProgress = resp.InstallProgress;
				} else if (resp.InstallComplete) {
					installProgress = { stage: 'Complete!', percent: 100 };
					installPolling = false;
					// Pre-fill config from install results
					if (resp.InstallComplete.game_log) {
						configGameLog = resp.InstallComplete.game_log;
					}
					if (resp.InstallComplete.install_path) {
						installPath = resp.InstallComplete.install_path;
					}
					// Move to manual config step with pre-filled game_log
					setupStep = 2;
				} else if (resp.Error) {
					installError = resp.Error.message || 'Installation failed';
					installPolling = false;
				}
			} catch (e) {
				installError = e.message || 'Lost connection during install';
				installPolling = false;
			}
			if (installPolling) {
				await new Promise(r => setTimeout(r, 2000));
			}
		}
	}

	/** Try to extract a human-readable message from an error thrown by the API client. */
	function extractErrorMessage(e, fallback) {
		const raw = e?.message || '';
		// The API may return JSON like {"ok":false,"message":"..."}
		try {
			const parsed = JSON.parse(raw);
			if (parsed.message) return parsed.message;
		} catch {}
		return raw || fallback;
	}

	// --- Scan flow ---
	async function scanConfigs() {
		scanning = true;
		scanError = '';
		scanResults = null;
		try {
			const resp = await api.scanServerConfigs(serverId);
			console.log('[R3] scanConfigs response:', resp);
			if (resp.response_type === 'ConfigFiles') {
				scanResults = resp.data?.files || [];
			} else if (resp.response_type === 'Error') {
				scanError = resp.data?.message || 'Scan failed';
			} else {
				scanError = `Unexpected response: ${JSON.stringify(resp).slice(0, 200)}`;
			}
		} catch (e) {
			console.error('[R3] scanConfigs error:', e);
			scanError = extractErrorMessage(e, 'Failed to scan for config files');
		}
		scanning = false;
	}

	// --- Browse flow ---
	async function browseFiles(path = '') {
		browsing = true;
		browseError = '';
		try {
			const resp = await api.browseServerFiles(serverId, path);
			console.log('[R3] browseFiles response:', resp);
			if (resp.response_type === 'DirectoryListing') {
				browseEntries = resp.data?.entries || [];
				browsePath = resp.data?.path || path;
			} else if (resp.response_type === 'Error') {
				browseError = resp.data?.message || 'Browse failed';
			} else {
				browseError = `Unexpected response: ${JSON.stringify(resp).slice(0, 200)}`;
			}
		} catch (e) {
			console.error('[R3] browseFiles error:', e);
			browseError = extractErrorMessage(e, 'Failed to browse files');
		}
		browsing = false;
	}

	function browseParent() {
		if (!browsePath || browsePath === '/') return;
		const parts = browsePath.split('/').filter(Boolean);
		parts.pop();
		const parent = '/' + parts.join('/');
		browseFiles(parent);
	}

	function selectBrowseFile(name) {
		const fullPath = browsePath.endsWith('/') ? browsePath + name : browsePath + '/' + name;
		selectedConfigPath = fullPath;
	}

	async function parseSelectedConfig() {
		if (!selectedConfigPath) return;
		parsing = true;
		parseError = '';
		parsedConfig = null;
		try {
			const resp = await api.parseServerConfig(serverId, selectedConfigPath);
			console.log('[R3] parseSelectedConfig response:', resp);
			if (resp.response_type === 'ParsedConfig') {
				parsedConfig = resp.data;
				// Pre-fill the config form
				if (parsedConfig.settings?.public_ip) configAddress = parsedConfig.settings.public_ip;
				if (parsedConfig.settings?.port) configPort = parsedConfig.settings.port;
				if (parsedConfig.settings?.rcon_password) configRconPassword = parsedConfig.settings.rcon_password;
				if (parsedConfig.settings?.game_log) configGameLog = parsedConfig.settings.game_log;
				setupStep = 2;
			} else if (resp.response_type === 'Error') {
				parseError = resp.data?.message || 'Parse failed';
			} else {
				parseError = `Unexpected response: ${JSON.stringify(resp).slice(0, 200)}`;
			}
		} catch (e) {
			console.error('[R3] parseSelectedConfig error:', e);
			parseError = extractErrorMessage(e, 'Failed to parse config file');
		}
		parsing = false;
	}

	function selectMethod(method) {
		setupMethod = method;
		setupStep = 1;
		// Reset sub-states
		installStarted = false;
		installProgress = null;
		installError = '';
		scanResults = null;
		scanError = '';
		selectedConfigPath = '';
		parsedConfig = null;
		parseError = '';
		browseEntries = null;
		browsePath = '';
		browseError = '';
		configResult = null;
	}

	function backToMethodSelect() {
		setupMethod = null;
		setupStep = 0;
	}

	function isUnconfigured() {
		return server && (!server.address || server.address === '0.0.0.0' || server.port === 0);
	}

	async function sendRcon() {
		if (!rconCommand.trim()) return;
		const cmd = rconCommand;
		rconCommand = '';
		rconSending = true;
		rconHistory = [...rconHistory, { type: 'cmd', text: cmd }];
		try {
			const res = await api.serverRcon(serverId, cmd);
			rconHistory = [...rconHistory, { type: 'ok', text: res.message }];
		} catch (e) {
			rconHistory = [...rconHistory, { type: 'err', text: e.message || 'Failed' }];
		}
		rconSending = false;
	}

	async function sendSay() {
		if (!sayMessage.trim()) return;
		saySending = true;
		sayResult = null;
		try {
			await api.serverSay(serverId, sayMessage);
			sayResult = { ok: true, message: 'Message sent' };
			sayMessage = '';
		} catch (e) {
			sayResult = { ok: false, message: e.message || 'Failed' };
		}
		saySending = false;
	}

	async function doAction() {
		if (!actionCid.trim()) return;
		actionSending = true;
		actionResult = null;
		try {
			if (actionType === 'kick') {
				await api.serverKick(serverId, actionCid, actionReason || undefined);
			} else {
				await api.serverBan(serverId, actionCid, actionReason || undefined, actionDuration);
			}
			actionResult = { ok: true, message: `${actionType === 'kick' ? 'Kick' : 'Ban'} sent` };
			actionCid = '';
			actionReason = '';
		} catch (e) {
			actionResult = { ok: false, message: e.message || 'Failed' };
		}
		actionSending = false;
	}

	async function loadVersion() {
		versionLoading = true;
		versionError = '';
		try {
			versionInfo = await api.serverVersion(serverId);
		} catch (e) {
			versionError = e.message || 'Failed to load version info';
			console.error('[R3] loadVersion failed', e);
		}
		versionLoading = false;
	}

	async function forceUpdate() {
		if (!confirm('Force this server to download and apply the latest build, then restart?\n\nThe client will be briefly offline while it updates.')) return;
		forceUpdating = true;
		forceUpdateResult = null;
		try {
			const res = await api.forceServerUpdate(serverId);
			forceUpdateResult = { ok: true, data: res };
			// Refresh version info after a short delay so the UI shows the new build
			setTimeout(() => loadVersion(), 15000);
		} catch (e) {
			forceUpdateResult = { ok: false, message: e.message || 'Force update failed' };
			console.error('[R3] forceUpdate failed', e);
		}
		forceUpdating = false;
	}

	$effect(() => { loadServer(); loadConfig(); loadVersion(); });
</script>

<div class="mx-auto max-w-5xl space-y-6">
	<!-- Back link -->
	<a href="/servers" class="inline-flex items-center gap-2 text-sm text-surface-500 hover:text-surface-300 transition-colors">
		<ArrowLeft class="h-4 w-4" />
		Back to Servers
	</a>

	{#if loading}
		<div class="flex justify-center py-12">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
		</div>
	{:else if error}
		<div class="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">{error}</div>
	{:else if server}
		<!-- Server header -->
		<div class="rounded-xl border border-surface-800 bg-surface-900 p-6">
			<div class="flex items-center gap-4">
				<div class="flex h-12 w-12 items-center justify-center rounded-lg {server.online ? 'bg-emerald-500/10' : 'bg-surface-800'}">
					{#if server.online}
						<Wifi class="h-6 w-6 text-emerald-400" />
					{:else}
						<WifiOff class="h-6 w-6 text-surface-500" />
					{/if}
				</div>
				<div class="flex-1">
					<h1 class="text-xl font-bold text-surface-100">{server.name}</h1>
					<div class="mt-1 flex items-center gap-4 text-sm text-surface-500">
						<span>{server.address}:{server.port}</span>
						<span class="rounded-full px-2 py-0.5 text-xs font-medium {server.online ? 'bg-emerald-500/10 text-emerald-400' : 'bg-surface-800 text-surface-500'}">
							{server.online ? 'Online' : 'Offline'}
						</span>
					</div>
				</div>
				<button onclick={() => { loading = true; loadServer(); }} class="btn-secondary flex items-center gap-2">
					<RefreshCw class="h-4 w-4" />
					Refresh
				</button>
			</div>

			{#if server.online}
				<div class="mt-5 grid grid-cols-2 gap-4 sm:grid-cols-4">
					<div class="rounded-lg bg-surface-800/50 p-3">
						<div class="text-xs text-surface-500">Players</div>
						<div class="mt-1 text-lg font-semibold text-surface-100">{server.player_count} <span class="text-sm text-surface-500">/ {server.max_clients}</span></div>
					</div>
					<div class="rounded-lg bg-surface-800/50 p-3">
						<div class="text-xs text-surface-500">Map</div>
						<div class="mt-1 text-lg font-semibold text-surface-100">{server.current_map || '—'}</div>
					</div>
					<div class="rounded-lg bg-surface-800/50 p-3">
						<div class="text-xs text-surface-500">Status</div>
						<div class="mt-1 text-lg font-semibold text-surface-100">{server.status}</div>
					</div>
					<div class="rounded-lg bg-surface-800/50 p-3">
						<div class="text-xs text-surface-500">Last Seen</div>
						<div class="mt-1 text-sm font-medium text-surface-200">{server.last_seen ? new Date(server.last_seen).toLocaleString() : '—'}</div>
					</div>
				</div>
			{/if}
		</div>

		<!-- Version & Updates -->
		<div class="rounded-xl border border-surface-800 bg-surface-900 p-6">
			<div class="flex items-center justify-between gap-4 mb-4">
				<h2 class="flex items-center gap-2 text-base font-semibold text-surface-100">
					<Download class="h-4 w-4" />
					Version & Updates
				</h2>
				<button onclick={loadVersion} class="btn-secondary flex items-center gap-2 text-sm" disabled={versionLoading}>
					<RefreshCw class="h-3 w-3 {versionLoading ? 'animate-spin' : ''}" />
					Refresh
				</button>
			</div>

			{#if versionError}
				<div class="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-2 text-xs text-red-400 mb-3">{versionError}</div>
			{/if}

			{#if versionInfo}
				{@const clientResp = versionInfo.client?.response}
				{@const clientVer = clientResp?.response_type === 'Version' ? clientResp.data : null}
				{@const cached = versionInfo.cached}
				{@const latest = versionInfo.latest?.ok && !versionInfo.latest?.up_to_date ? versionInfo.latest : null}
				{@const currentBuild = clientVer?.build_hash || cached?.build_hash || null}
				{@const latestBuild = latest?.build_hash || null}
				{@const upToDate = currentBuild && latestBuild && currentBuild === latestBuild}
				{@const updateAvailable = currentBuild && latestBuild && currentBuild !== latestBuild}

				<div class="grid gap-4 sm:grid-cols-2">
					<div class="rounded-lg bg-surface-800/50 p-3">
						<div class="text-xs text-surface-500 mb-1">Client Build</div>
						{#if clientVer}
							<div class="text-sm font-mono text-surface-100">{clientVer.build_hash}</div>
							<div class="text-xs text-surface-500 mt-1">v{clientVer.version} · {clientVer.platform}</div>
						{:else if cached}
							<div class="text-sm font-mono text-surface-300">{cached.build_hash || '—'}</div>
							<div class="text-xs text-surface-500 mt-1">
								{cached.version ? `v${cached.version} · ` : ''}offline — last heartbeat {new Date(cached.reported_at).toLocaleString()}
							</div>
						{:else}
							<div class="text-sm text-surface-500">Client did not respond — server may be offline or running an older build.</div>
							{#if versionInfo.client?.error}
								<div class="text-xs text-surface-500 mt-1">{versionInfo.client.error}</div>
							{/if}
						{/if}
					</div>

					<div class="rounded-lg bg-surface-800/50 p-3">
						<div class="text-xs text-surface-500 mb-1">Latest Available</div>
						{#if latest}
							<div class="text-sm font-mono text-surface-100">{latest.build_hash}</div>
							<div class="text-xs text-surface-500 mt-1">
								v{latest.version} · {(latest.download_size / 1024 / 1024).toFixed(1)} MB
								{#if latest.released_at} · {new Date(latest.released_at).toLocaleDateString()}{/if}
							</div>
						{:else if versionInfo.latest?.up_to_date}
							<div class="text-sm text-surface-300">No newer build published</div>
						{:else if versionInfo.latest?.error}
							<div class="text-sm text-red-400">{versionInfo.latest.error}</div>
						{:else}
							<div class="text-sm text-surface-500">—</div>
						{/if}
					</div>
				</div>

				<div class="mt-4 flex items-center justify-between gap-3 flex-wrap">
					<div class="text-sm">
						{#if upToDate}
							<span class="inline-flex items-center gap-1 text-emerald-400"><Check class="h-4 w-4" /> Up to date</span>
						{:else if updateAvailable}
							<span class="inline-flex items-center gap-1 text-amber-400"><AlertTriangle class="h-4 w-4" /> Update available</span>
						{:else}
							<span class="text-surface-500">Status unknown</span>
						{/if}
					</div>
					<button
						onclick={forceUpdate}
						disabled={forceUpdating || !server.online}
						class="flex items-center gap-2 rounded-lg bg-amber-600 px-4 py-2 text-sm font-medium text-white hover:bg-amber-700 transition-colors disabled:opacity-50"
						title={!server.online ? 'Client must be online to force an update' : 'Download & apply latest build on this client'}
					>
						{#if forceUpdating}
							<Loader2 class="h-4 w-4 animate-spin" />
							Requesting update...
						{:else}
							<Download class="h-4 w-4" />
							Force Update
						{/if}
					</button>
				</div>

				{#if forceUpdateResult}
					<div class="mt-3 rounded-lg px-3 py-2 text-xs {forceUpdateResult.ok ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20' : 'bg-red-500/10 text-red-400 border border-red-500/20'}">
						{#if forceUpdateResult.ok}
							{#if forceUpdateResult.data?.response_type === 'UpdateTriggered'}
								Update triggered: {forceUpdateResult.data.data.current_build} → {forceUpdateResult.data.data.target_build}. The client is downloading and will restart shortly.
							{:else if forceUpdateResult.data?.response_type === 'AlreadyUpToDate'}
								Client reports it is already up to date ({forceUpdateResult.data.data.current_build}).
							{:else if forceUpdateResult.data?.response_type === 'Error'}
								Client error: {forceUpdateResult.data.data.message}
							{:else}
								Request accepted.
							{/if}
						{:else}
							{forceUpdateResult.message}
						{/if}
					</div>
				{/if}
			{:else if versionLoading}
				<div class="text-sm text-surface-500 flex items-center gap-2"><Loader2 class="h-4 w-4 animate-spin" /> Loading version info...</div>
			{:else}
				<div class="text-sm text-surface-500">No version info yet.</div>
			{/if}
		</div>

		{#if isUnconfigured()}
			<!-- Setup Wizard -->
			{#if setupMethod === null}
				<!-- Method Selection -->
				<div class="rounded-xl border border-amber-500/30 bg-amber-500/5 p-6">
					<h2 class="mb-2 flex items-center gap-2 text-base font-semibold text-amber-400">
						<Settings class="h-4 w-4" />
						Game Server Configuration Required
					</h2>
					<p class="text-sm text-surface-400">This client has connected but the game server hasn't been configured yet. Choose how you'd like to set it up:</p>
				</div>

				<div class="grid gap-4 sm:grid-cols-3">
					<!-- Install Fresh -->
					<button onclick={() => selectMethod('install')} class="group rounded-xl border border-surface-700 bg-surface-900 p-6 text-left hover:border-accent/50 hover:bg-surface-800/80 transition-all">
						<div class="mb-3 flex h-10 w-10 items-center justify-center rounded-lg bg-blue-500/10">
							<Download class="h-5 w-5 text-blue-400" />
						</div>
						<h3 class="text-sm font-semibold text-surface-100">Install Fresh Copy</h3>
						<p class="mt-1 text-xs text-surface-500">Download and install a fresh Urban Terror 4.3 dedicated server on the client machine.</p>
					</button>

					<!-- Scan Config -->
					<button onclick={() => selectMethod('scan')} class="group rounded-xl border border-surface-700 bg-surface-900 p-6 text-left hover:border-accent/50 hover:bg-surface-800/80 transition-all">
						<div class="mb-3 flex h-10 w-10 items-center justify-center rounded-lg bg-emerald-500/10">
							<FileSearch class="h-5 w-5 text-emerald-400" />
						</div>
						<h3 class="text-sm font-semibold text-surface-100">Scan Config File</h3>
						<p class="mt-1 text-xs text-surface-500">Scan the client machine for existing server config files and auto-detect settings.</p>
					</button>

					<!-- Manual -->
					<button onclick={() => selectMethod('manual')} class="group rounded-xl border border-surface-700 bg-surface-900 p-6 text-left hover:border-accent/50 hover:bg-surface-800/80 transition-all">
						<div class="mb-3 flex h-10 w-10 items-center justify-center rounded-lg bg-purple-500/10">
							<Wrench class="h-5 w-5 text-purple-400" />
						</div>
						<h3 class="text-sm font-semibold text-surface-100">Manual Configuration</h3>
						<p class="mt-1 text-xs text-surface-500">Manually enter the game server IP, port, RCON password, and log path.</p>
					</button>
				</div>

			{:else if setupMethod === 'install' && setupStep === 1}
				<!-- Install Flow -->
				<div class="rounded-xl border border-surface-800 bg-surface-900 p-6">
					<div class="mb-4 flex items-center justify-between">
						<h2 class="flex items-center gap-2 text-base font-semibold text-surface-100">
							<Download class="h-4 w-4 text-blue-400" />
							Install Game Server
						</h2>
						<button onclick={backToMethodSelect} class="text-xs text-surface-500 hover:text-surface-300 transition-colors">&larr; Back</button>
					</div>

					{#if !installStarted}
						<p class="mb-4 text-sm text-surface-400">Choose a directory on the client machine where the Urban Terror 4.3 dedicated server will be installed.</p>
						<div class="mb-4">
							<label for="install-path" class="mb-1 block text-xs font-medium text-surface-400">Install Directory</label>
							<input id="install-path" type="text" bind:value={installPath} placeholder="/opt/urbanterror" class="input text-sm" />
						</div>
						<button onclick={startInstall} class="btn-primary flex items-center gap-2" disabled={!installPath.trim()}>
							<Download class="h-4 w-4" />
							Start Installation
						</button>
					{:else}
						<!-- Progress -->
						<div class="space-y-3">
							<div class="flex items-center gap-3">
								{#if installError}
									<AlertTriangle class="h-5 w-5 text-red-400" />
								{:else if installProgress?.percent >= 100}
									<Check class="h-5 w-5 text-emerald-400" />
								{:else}
									<Loader2 class="h-5 w-5 animate-spin text-accent" />
								{/if}
								<span class="text-sm text-surface-200">{installProgress?.stage || 'Starting...'}</span>
							</div>

							<div class="h-2 w-full overflow-hidden rounded-full bg-surface-800">
								<div class="h-full rounded-full bg-accent transition-all duration-500" style="width: {installProgress?.percent || 0}%"></div>
							</div>

							{#if installError}
								<div class="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-2 text-sm text-red-400">{installError}</div>
								<button onclick={() => { installStarted = false; installError = ''; }} class="btn-secondary text-sm">Try Again</button>
							{/if}
						</div>
					{/if}
				</div>

			{:else if setupMethod === 'scan' && setupStep === 1}
				<!-- Scan / Browse Flow -->
				<div class="rounded-xl border border-surface-800 bg-surface-900 p-6">
					<div class="mb-4 flex items-center justify-between">
						<h2 class="flex items-center gap-2 text-base font-semibold text-surface-100">
							<FileSearch class="h-4 w-4 text-emerald-400" />
							Find Config File
						</h2>
						<button onclick={backToMethodSelect} class="text-xs text-surface-500 hover:text-surface-300 transition-colors">&larr; Back</button>
					</div>

					<p class="mb-4 text-sm text-surface-400">Browse the client's filesystem to locate a server.cfg file, or auto-scan common directories.</p>

					<!-- Action buttons -->
					<div class="mb-4 flex items-center gap-3">
						<button onclick={() => browseFiles('')} class="btn-secondary flex items-center gap-2 text-sm" disabled={browsing}>
							{#if browsing}
								<Loader2 class="h-4 w-4 animate-spin" />
							{:else}
								<FolderOpen class="h-4 w-4" />
							{/if}
							Browse Files
						</button>
						<button onclick={scanConfigs} class="btn-secondary flex items-center gap-2 text-sm" disabled={scanning}>
							{#if scanning}
								<Loader2 class="h-4 w-4 animate-spin" />
							{:else}
								<FileSearch class="h-4 w-4" />
							{/if}
							Auto-Scan
						</button>
					</div>

					<!-- File Browser -->
					{#if browseEntries !== null}
						<div class="rounded-lg border border-surface-700 bg-surface-800/50">
							<!-- Path bar -->
							<div class="flex items-center gap-2 border-b border-surface-700 px-3 py-2">
								<Folder class="h-4 w-4 text-surface-400 flex-shrink-0" />
								<span class="text-xs font-mono text-surface-300 truncate">{browsePath}</span>
								{#if browsePath && browsePath !== '/'}
									<button onclick={browseParent} class="ml-auto text-xs text-accent hover:text-accent/80 transition-colors flex-shrink-0">&larr; Up</button>
								{/if}
							</div>
							<!-- Entries -->
							<div class="max-h-64 overflow-y-auto divide-y divide-surface-700/50">
								{#if browseEntries.length === 0}
									<div class="px-3 py-4 text-center text-xs text-surface-500">No .cfg files or folders found here</div>
								{:else}
									{#each browseEntries as entry}
										{#if entry.is_dir}
											<button
												onclick={() => browseFiles(browsePath.endsWith('/') ? browsePath + entry.name : browsePath + '/' + entry.name)}
												class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-surface-700/50 transition-colors"
											>
												<Folder class="h-4 w-4 text-amber-400 flex-shrink-0" />
												<span class="text-surface-200 truncate">{entry.name}</span>
												<ChevronRight class="ml-auto h-3.5 w-3.5 text-surface-600 flex-shrink-0" />
											</button>
										{:else}
											<button
												onclick={() => selectBrowseFile(entry.name)}
												class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm transition-colors {selectedConfigPath === (browsePath.endsWith('/') ? browsePath + entry.name : browsePath + '/' + entry.name) ? 'bg-accent/10 text-accent' : 'hover:bg-surface-700/50 text-surface-300'}"
											>
												<FileText class="h-4 w-4 text-emerald-400 flex-shrink-0" />
												<span class="truncate">{entry.name}</span>
												<span class="ml-auto text-xs text-surface-500 flex-shrink-0">{(entry.size / 1024).toFixed(1)} KB</span>
											</button>
										{/if}
									{/each}
								{/if}
							</div>
						</div>
					{/if}

					<!-- Auto-Scan Results -->
					{#if scanResults !== null}
						{#if scanResults.length === 0}
							<div class="mt-3 rounded-lg bg-amber-500/10 border border-amber-500/20 px-3 py-2 text-sm text-amber-400">
								No config files found in common directories. Try browsing manually.
							</div>
						{:else}
							<div class="mt-3">
								<p class="mb-2 text-xs font-medium text-surface-400">Found {scanResults.length} config file{scanResults.length !== 1 ? 's' : ''} in common directories:</p>
								<div class="space-y-1 max-h-48 overflow-y-auto">
									{#each scanResults as file}
										<button
											onclick={() => { selectedConfigPath = file.path; }}
											class="w-full rounded-lg border px-3 py-2 text-left text-sm transition-colors {selectedConfigPath === file.path ? 'border-accent bg-accent/10 text-accent' : 'border-surface-700 bg-surface-800 text-surface-300 hover:border-surface-600'}"
										>
											<div class="font-mono text-xs">{file.path}</div>
											{#if file.size}
												<div class="mt-0.5 text-xs text-surface-500">{(file.size / 1024).toFixed(1)} KB</div>
											{/if}
										</button>
									{/each}
								</div>
							</div>
						{/if}
					{/if}

					<!-- Selected file + parse button -->
					{#if selectedConfigPath}
						<div class="mt-4 flex items-center gap-3">
							<div class="flex-1 rounded-lg border border-accent/30 bg-accent/5 px-3 py-2">
								<div class="text-xs text-surface-400">Selected config file</div>
								<div class="font-mono text-sm text-accent truncate">{selectedConfigPath}</div>
							</div>
							<button onclick={parseSelectedConfig} class="btn-primary flex items-center gap-2 flex-shrink-0" disabled={parsing}>
								{#if parsing}
									<Loader2 class="h-4 w-4 animate-spin" />
									Parsing...
								{:else}
									<Check class="h-4 w-4" />
									Use This File
								{/if}
							</button>
						</div>
					{/if}

					{#if browseError}
						<div class="mt-3 rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-2 text-sm text-red-400">{browseError}</div>
					{/if}
					{#if scanError}
						<div class="mt-3 rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-2 text-sm text-red-400">{scanError}</div>
					{/if}
					{#if parseError}
						<div class="mt-3 rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-2 text-sm text-red-400">{parseError}</div>
					{/if}
				</div>

			{:else if setupStep === 2 || setupMethod === 'manual'}
				<!-- Final config form (pre-filled from scan/install, or blank for manual) -->
				<div class="rounded-xl border border-surface-800 bg-surface-900 p-6">
					<div class="mb-4 flex items-center justify-between">
						<h2 class="flex items-center gap-2 text-base font-semibold text-surface-100">
							<Settings class="h-4 w-4 text-surface-400" />
							{#if setupMethod === 'scan'}
								Confirm Detected Settings
							{:else if setupMethod === 'install'}
								Configure Installed Server
							{:else}
								Manual Configuration
							{/if}
						</h2>
						<button onclick={backToMethodSelect} class="text-xs text-surface-500 hover:text-surface-300 transition-colors">&larr; Back</button>
					</div>

					{#if parsedConfig?.checks?.length > 0}
						<div class="mb-4 space-y-2">
							{#each parsedConfig.checks as check}
								<div class="flex items-start gap-2 rounded-lg px-3 py-2 text-xs {check.ok ? 'bg-emerald-500/10 text-emerald-400' : 'bg-amber-500/10 text-amber-400'}">
									{#if check.ok}
										<Check class="mt-0.5 h-3.5 w-3.5 flex-shrink-0" />
									{:else}
										<AlertTriangle class="mt-0.5 h-3.5 w-3.5 flex-shrink-0" />
									{/if}
									<div>
										<span class="font-medium">{check.name}</span>
										{#if check.message}<span class="ml-1 text-surface-400">— {check.message}</span>{/if}
									</div>
								</div>
							{/each}
						</div>
					{/if}

					{#if configResult}
						<div class="mb-4 rounded-lg px-3 py-2 text-sm {configResult.ok ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}">
							{configResult.message}
						</div>
					{/if}

					<div class="grid gap-4 sm:grid-cols-2">
						<div>
							<label for="cfg-address" class="mb-1 block text-xs font-medium text-surface-400">Server IP Address</label>
							<input id="cfg-address" type="text" bind:value={configAddress} placeholder="e.g. 203.0.113.10" class="input text-sm" />
						</div>
						<div>
							<label for="cfg-port" class="mb-1 block text-xs font-medium text-surface-400">Game Port</label>
							<input id="cfg-port" type="number" bind:value={configPort} placeholder="27960" class="input text-sm" />
						</div>
						<div>
							<label for="cfg-rcon" class="mb-1 block text-xs font-medium text-surface-400">RCON Password</label>
							<input id="cfg-rcon" type="password" bind:value={configRconPassword} placeholder="RCON password" class="input text-sm" />
						</div>
						<div>
							<label for="cfg-log" class="mb-1 block text-xs font-medium text-surface-400">Game Log Path <span class="text-surface-600">(optional)</span></label>
							<input id="cfg-log" type="text" bind:value={configGameLog} placeholder="/path/to/games.log" class="input text-sm" />
						</div>
					</div>

					<div class="mt-4 flex items-center gap-3">
						<button onclick={saveConfig} class="btn-primary flex items-center gap-2" disabled={configSaving || !configAddress.trim() || !configRconPassword.trim()}>
							<Save class="h-4 w-4" />
							{configSaving ? 'Saving...' : 'Save & Push to Client'}
						</button>
						{#if configSaving}
							<div class="h-4 w-4 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
						{/if}
					</div>
				</div>
			{/if}
		{:else}
			<!-- Already configured — show editable config -->
			<div class="rounded-xl border border-surface-800 bg-surface-900 p-6">
				<h2 class="mb-4 flex items-center gap-2 text-base font-semibold text-surface-100">
					<Settings class="h-4 w-4 text-surface-400" />
					Game Server Settings
				</h2>

				{#if configResult}
					<div class="mb-4 rounded-lg px-3 py-2 text-sm {configResult.ok ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}">
						{configResult.message}
					</div>
				{/if}

				<div class="grid gap-4 sm:grid-cols-2">
					<div>
						<label for="cfg-address" class="mb-1 block text-xs font-medium text-surface-400">Server IP Address</label>
						<input id="cfg-address" type="text" bind:value={configAddress} placeholder="e.g. 203.0.113.10" class="input text-sm" />
					</div>
					<div>
						<label for="cfg-port" class="mb-1 block text-xs font-medium text-surface-400">Game Port</label>
						<input id="cfg-port" type="number" bind:value={configPort} placeholder="27960" class="input text-sm" />
					</div>
					<div>
						<label for="cfg-rcon" class="mb-1 block text-xs font-medium text-surface-400">RCON Password</label>
						<input id="cfg-rcon" type="password" bind:value={configRconPassword} placeholder="RCON password" class="input text-sm" />
					</div>
					<div>
						<label for="cfg-log" class="mb-1 block text-xs font-medium text-surface-400">Game Log Path <span class="text-surface-600">(optional)</span></label>
						<input id="cfg-log" type="text" bind:value={configGameLog} placeholder="/path/to/games.log" class="input text-sm" />
					</div>
				</div>

				<div class="mt-4 flex items-center gap-3">
					<button onclick={saveConfig} class="btn-primary flex items-center gap-2" disabled={configSaving || !configAddress.trim() || !configRconPassword.trim()}>
						<Save class="h-4 w-4" />
						{configSaving ? 'Saving...' : 'Save & Push to Client'}
					</button>
					{#if configSaving}
						<div class="h-4 w-4 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
					{/if}
				</div>
			</div>
		{/if}

		{#if server.online}
			<!-- RCON Console -->
			<div class="rounded-xl border border-surface-800 bg-surface-900 p-6">
				<h2 class="mb-4 flex items-center gap-2 text-base font-semibold text-surface-100">
					<Terminal class="h-4 w-4 text-accent" />
					Remote Console
				</h2>

				{#if rconHistory.length > 0}
					<div class="mb-3 max-h-64 overflow-y-auto rounded-lg bg-surface-950 p-3 font-mono text-xs">
						{#each rconHistory as entry}
							{#if entry.type === 'cmd'}
								<div class="text-accent">&gt; {entry.text}</div>
							{:else if entry.type === 'ok'}
								<div class="text-surface-300">{entry.text}</div>
							{:else}
								<div class="text-red-400">{entry.text}</div>
							{/if}
						{/each}
					</div>
				{/if}

				<div class="flex gap-2">
					<input type="text" bind:value={rconCommand} placeholder="Enter RCON command..." class="input flex-1 font-mono text-sm"
						onkeydown={(e) => { if (e.key === 'Enter') sendRcon(); }} />
					<button onclick={sendRcon} class="btn-primary" disabled={rconSending || !rconCommand.trim()}>
						<Send class="h-4 w-4" />
					</button>
				</div>
			</div>

			<!-- Say & Actions -->
			<div class="grid gap-4 md:grid-cols-2">
				<!-- Broadcast -->
				<div class="rounded-xl border border-surface-800 bg-surface-900 p-6">
					<h2 class="mb-4 flex items-center gap-2 text-base font-semibold text-surface-100">
						<MessageSquare class="h-4 w-4 text-blue-400" />
						Broadcast Message
					</h2>

					{#if sayResult}
						<div class="mb-3 rounded-lg px-3 py-2 text-xs {sayResult.ok ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}">
							{sayResult.message}
						</div>
					{/if}

					<div class="flex gap-2">
						<input type="text" bind:value={sayMessage} placeholder="Message..." class="input flex-1 text-sm"
							onkeydown={(e) => { if (e.key === 'Enter') sendSay(); }} />
						<button onclick={sendSay} class="btn-primary" disabled={saySending || !sayMessage.trim()}>Send</button>
					</div>
				</div>

				<!-- Kick/Ban -->
				<div class="rounded-xl border border-surface-800 bg-surface-900 p-6">
					<h2 class="mb-4 flex items-center gap-2 text-base font-semibold text-surface-100">
						<ShieldBan class="h-4 w-4 text-red-400" />
						Kick / Ban Player
					</h2>

					{#if actionResult}
						<div class="mb-3 rounded-lg px-3 py-2 text-xs {actionResult.ok ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}">
							{actionResult.message}
						</div>
					{/if}

					<div class="space-y-3">
						<input type="text" bind:value={actionCid} placeholder="Player slot ID" class="input text-sm" />
						<input type="text" bind:value={actionReason} placeholder="Reason (optional)" class="input text-sm" />
						<div class="flex gap-2">
							<button onclick={() => { actionType = 'kick'; doAction(); }} class="btn-secondary flex-1 text-sm" disabled={actionSending || !actionCid.trim()}>
								<UserX class="mr-1 inline h-3.5 w-3.5" /> Kick
							</button>
							<button onclick={() => { actionType = 'ban'; doAction(); }} class="flex-1 rounded-lg bg-red-600 px-4 py-2 text-sm font-medium text-white hover:bg-red-700 transition-colors disabled:opacity-50" disabled={actionSending || !actionCid.trim()}>
								<ShieldBan class="mr-1 inline h-3.5 w-3.5" /> Ban
							</button>
						</div>
					</div>
				</div>
			</div>
		{/if}
	{/if}
</div>

<style>
	.input { @apply w-full rounded-lg border border-surface-700 bg-surface-800 px-3 py-2 text-surface-100 placeholder-surface-600 focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent; }
	.btn-primary { @apply rounded-lg bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent/90 transition-colors disabled:opacity-50; }
	.btn-secondary { @apply rounded-lg border border-surface-700 bg-surface-800 px-4 py-2 text-sm font-medium text-surface-300 hover:bg-surface-700 transition-colors disabled:opacity-50; }
</style>
