<script>
	import { page } from '$app/stores';
	import { api } from '$lib/api.svelte.js';
	import {
		HardDrive, RefreshCw, ArrowLeft, Cpu, MemoryStick, HardDriveDownload,
		Play, Square, RotateCw, Trash2, Plus, Terminal, Wifi, WifiOff,
		Download, CheckCircle2, AlertCircle, Sliders
	} from 'lucide-svelte';

	const hubId = $derived(Number($page.params.id));

	let hub = $state(null);
	let hostInfo = $state(null);
	let clients = $state([]);
	let metrics = $state([]);
	let metricsRange = $state('1h');
	let loading = $state(true);
	let error = $state('');
	let refreshing = $state(false);
	let tab = $state('clients');

	// Install-client form state
	let showInstall = $state(false);
	let installSlug = $state('');
	let installServerName = $state('');
	let installBusy = $state(false);
	let installError = $state('');

	// Optional game server install alongside the client
	let installGameServer = $state(true);
	let showAdvanced = $state(false);
	let gsInstallPath = $state('');
	let gsPublicIp = $state('');
	let gsPort = $state(27960);
	let gsGameMode = $state('CTF');
	let gsMaxClients = $state(16);
	let gsForceDownload = $state(false);

	// Live install-progress state
	let installActionId = $state('');
	let installProgress = $state(/** @type {{step:string,message:string,percent:number|null}[]} */ ([]));
	let installPercent = $state(0);
	let installResult = $state(/** @type {{ok:boolean,message:string}|null} */ (null));
	let installPollTimer = /** @type {any} */ (null);

	/** Generate a short random hex password (6 chars). */
	function randomPass() {
		const buf = new Uint8Array(3);
		(globalThis.crypto ?? window.crypto).getRandomValues(buf);
		return Array.from(buf, (b) => b.toString(16).padStart(2, '0')).join('');
	}

	const GAME_MODES = [
		{ value: 'FFA', label: 'Free for All' },
		{ value: 'LMS', label: 'Last Man Standing' },
		{ value: 'TDM', label: 'Team Deathmatch' },
		{ value: 'TS', label: 'Team Survivor' },
		{ value: 'FTL', label: 'Follow the Leader' },
		{ value: 'CAH', label: 'Capture and Hold' },
		{ value: 'CTF', label: 'Capture the Flag' },
		{ value: 'BOMB', label: 'Bomb' },
		{ value: 'JUMP', label: 'Jump' },
		{ value: 'FREEZE', label: 'Freeze Tag' },
		{ value: 'GUNGAME', label: 'Gun Game' },
	];

	// Per-client action busy flags
	let busySlug = $state('');

	// Reconfigure-game-server modal state.
	let showReconfig = $state(false);
	let reconfigSlug = $state('');
	let reconfigPort = $state(27960);
	let reconfigNetIp = $state('');
	let reconfigExtraArgsText = $state('');
	let reconfigActionId = $state('');
	let reconfigProgress = $state(/** @type {{step:string,ok:boolean,message:string}[]} */ ([]));
	let reconfigResult = $state(/** @type {{ok:boolean,message:string,data?:any}|null} */ (null));
	let reconfigBusy = $state(false);
	let reconfigError = $state('');
	let reconfigPollTimer = /** @type {any} */ (null);

	// Update card state
	let versionInfo = $state(null);
	let versionLoading = $state(false);
	let versionError = $state('');
	let forceUpdating = $state(false);
	let forceUpdateResult = $state(null);
	let channelSaving = $state(false);
	let channelResult = $state(null);
	let channelValue = $state('beta');
	let intervalDraft = $state('3600');
	let intervalSaving = $state(false);
	let intervalResult = $state(null);
	let updateEnabled = $state(true);
	let updateEnabledSaving = $state(false);
	let updateEnabledResult = $state(null);

	const CHANNELS = ['production', 'beta', 'alpha', 'dev'];

	async function load() {
		try {
			const d = await api.hub(hubId);
			hub = d.hub;
			hostInfo = d.host_info;
			clients = d.clients ?? [];
			error = '';
		} catch (e) {
			error = e.message || 'Failed to load hub';
		}
		loading = false;
	}

	async function loadMetrics() {
		try {
			const d = await api.hubMetrics(hubId, metricsRange);
			metrics = d.samples ?? d.metrics ?? d ?? [];
		} catch (e) {
			metrics = [];
		}
	}

	async function refresh() {
		refreshing = true;
		await Promise.all([load(), loadMetrics()]);
		refreshing = false;
	}

	async function clientAction(slug, action) {
		busySlug = slug + ':' + action;
		try {
			await api.hubClientAction(hubId, slug, action);
			await load();
		} catch (e) {
			error = e.message || `Failed to ${action} ${slug}`;
		}
		busySlug = '';
	}

	async function uninstallClient(slug) {
		if (!confirm(`Uninstall client "${slug}"? This removes its data from the hub and deletes the game server files.`)) return;
		busySlug = slug + ':uninstall';
		try {
			const resp = await api.hubUninstallClient(hubId, slug, true);
			// Non-blocking 202: reuse the install-progress panel to show steps.
			if (resp && resp.action_id) {
				showInstall = true;
				installActionId = resp.action_id;
				installProgress = [];
				installPercent = 0;
				installResult = null;
				installError = '';
				installBusy = true;
				await pollInstallProgress();
			} else {
				await load();
			}
		} catch (e) {
			error = e.message || `Failed to uninstall ${slug}`;
		}
		busySlug = '';
	}

	async function submitInstall() {
		installError = '';
		if (!installSlug.trim()) { installError = 'Slug is required'; return; }
		const slug = installSlug.trim();
		const serverName = installServerName.trim() || slug;

		/** @type {any} */
		const body = {
			slug,
			server_name: serverName,
			register_systemd: true
		};

		if (installGameServer) {
			if (!gsPort || gsPort < 1 || gsPort > 65535) { installError = 'Port must be 1–65535'; return; }
			// Auto-fill fields admins rarely need to touch.
			const installPath = gsInstallPath.trim() || `/home/urt/urt-${slug}`;
			const publicIp = gsPublicIp.trim(); // empty → hub auto-detects
			body.game_server = {
				install_path: installPath,
				hostname: serverName,
				public_ip: publicIp,
				port: Number(gsPort),
				rcon_password: randomPass(),
				admin_password: randomPass(),
				game_mode: gsGameMode,
				max_clients: Number(gsMaxClients),
				register_systemd: true,
				slug,
				force_download: gsForceDownload
			};
		}

		installBusy = true;
		installProgress = [];
		installPercent = 0;
		installResult = null;
		installActionId = '';
		try {
			const resp = await api.hubInstallClient(hubId, body);
			installActionId = resp?.action_id || '';
			if (!installActionId) throw new Error('Server did not return an action_id');
			await pollInstallProgress();
		} catch (e) {
			installError = e.message || 'Install failed';
			installBusy = false;
		}
	}

	async function pollInstallProgress() {
		if (!installActionId) return;
		try {
			const data = await api.hubActionProgress(hubId, installActionId);
			installProgress = data?.events ?? [];
			const last = installProgress[installProgress.length - 1];
			if (last && typeof last.percent === 'number') installPercent = last.percent;
			if (data?.done) {
				installResult = data.result || { ok: false, message: 'Unknown result' };
				installBusy = false;
				if (installResult?.ok) {
					// Refresh clients list after a successful install. Keep the
					// progress panel visible so the admin can see the outcome.
					installPercent = 100;
					await load();
				}
				return;
			}
		} catch (e) {
			// Transient network errors are non-fatal; just try again.
			console.warn('progress poll failed', e);
		}
		installPollTimer = setTimeout(pollInstallProgress, 1000);
	}

	function dismissInstallProgress() {
		if (installPollTimer) { clearTimeout(installPollTimer); installPollTimer = null; }
		installActionId = '';
		installProgress = [];
		installPercent = 0;
		installResult = null;
		installError = '';
		installSlug = '';
		installServerName = '';
		installGameServer = true;
		showAdvanced = false;
		gsInstallPath = '';
		gsPublicIp = '';
		gsPort = 27960;
		gsMaxClients = 16;
		gsGameMode = 'CTF';
		gsForceDownload = false;
		showInstall = false;
	}

	async function restartHub() {
		if (!confirm('Restart the hub process? Clients keep running, but the hub will briefly disconnect.')) return;
		try {
			await api.hubRestart(hubId);
		} catch (e) {
			error = e.message || 'Failed to restart hub';
		}
	}

	async function loadVersion() {
		versionLoading = true;
		versionError = '';
		try {
			versionInfo = await api.hubVersion(hubId);
			if (versionInfo?.channel) channelValue = versionInfo.channel;
			if (versionInfo?.update_interval) intervalDraft = String(versionInfo.update_interval);
			if (typeof versionInfo?.update_enabled === 'boolean') updateEnabled = versionInfo.update_enabled;
		} catch (e) {
			versionError = e.message || 'Failed to load version';
		}
		versionLoading = false;
	}

	async function forceUpdate() {
		if (!confirm('Force this hub to download and apply the latest build for its channel? The hub will restart.')) return;
		forceUpdating = true;
		forceUpdateResult = null;
		try {
			const r = await api.forceHubUpdate(hubId);
			forceUpdateResult = r;
		} catch (e) {
			forceUpdateResult = { ok: false, error: e.message || 'Force update failed' };
		}
		forceUpdating = false;
		// Refresh version info after a short delay so the UI reflects the new build.
		setTimeout(loadVersion, 3000);
	}

	async function changeUpdateChannel() {
		channelSaving = true;
		channelResult = null;
		try {
			const r = await api.setHubUpdateChannel(hubId, channelValue);
			channelResult = r;
			await loadVersion();
		} catch (e) {
			channelResult = { ok: false, error: e.message || 'Failed to update channel' };
		}
		channelSaving = false;
	}

	async function saveUpdateInterval() {
		const parsed = Math.floor(Number(intervalDraft));
		if (!Number.isFinite(parsed) || parsed < 60 || parsed > 604800) {
			intervalResult = { ok: false, error: 'Enter a value between 60 and 604800 seconds.' };
			return;
		}
		if (versionInfo?.update_interval && parsed === versionInfo.update_interval) {
			intervalResult = { ok: true, message: 'No change.' };
			return;
		}
		intervalSaving = true;
		intervalResult = null;
		try {
			const r = await api.setHubUpdateInterval(hubId, parsed);
			intervalResult = r;
			await loadVersion();
		} catch (e) {
			intervalResult = { ok: false, error: e.message || 'Failed to update interval' };
		}
		intervalSaving = false;
	}

	async function toggleUpdateEnabled() {
		const next = !updateEnabled;
		updateEnabledSaving = true;
		updateEnabledResult = null;
		try {
			const r = await api.setHubUpdateEnabled(hubId, next);
			updateEnabled = next;
			updateEnabledResult = r;
		} catch (e) {
			updateEnabledResult = { ok: false, error: e.message || 'Failed to toggle auto-update' };
		}
		updateEnabledSaving = false;
	}

	/** Open the Configure-Game-Server modal for a given client row. */
	function openReconfig(c) {
		reconfigSlug = c.slug;
		reconfigPort = c.port || 27960;
		reconfigNetIp = (c.address && c.address !== '0.0.0.0') ? c.address : '';
		reconfigExtraArgsText = '';
		reconfigProgress = [];
		reconfigResult = null;
		reconfigActionId = '';
		reconfigError = '';
		reconfigBusy = false;
		showReconfig = true;
	}

	function dismissReconfig() {
		if (reconfigPollTimer) { clearTimeout(reconfigPollTimer); reconfigPollTimer = null; }
		showReconfig = false;
		reconfigActionId = '';
		reconfigProgress = [];
		reconfigResult = null;
		reconfigError = '';
		reconfigBusy = false;
	}

	/**
	 * Split a free-form "+set x y +set a b" input into tokens the backend
	 * expects. Does not support quoted values — the backend rejects spaces
	 * inside tokens anyway, so keep it simple.
	 */
	function parseExtraArgs(text) {
		return text.trim().split(/\s+/).filter(Boolean);
	}

	async function submitReconfig() {
		reconfigError = '';
		const port = Number(reconfigPort);
		if (!Number.isInteger(port) || port < 1 || port > 65535) {
			reconfigError = 'Port must be an integer between 1 and 65535';
			return;
		}
		const netIp = reconfigNetIp.trim();
		if (netIp && !/^[0-9.]+$|^[0-9a-fA-F:]+$/.test(netIp)) {
			reconfigError = 'Bind IP must be a dotted IPv4 or IPv6 address (leave blank for bind-all)';
			return;
		}
		const extraArgs = parseExtraArgs(reconfigExtraArgsText);
		const bad = extraArgs.find(t => /[`$;&|<>"'\\(){}\[\]*?#!\s]/.test(t));
		if (bad) {
			reconfigError = `Extra arg "${bad}" contains a disallowed character`;
			return;
		}
		reconfigBusy = true;
		reconfigProgress = [];
		reconfigResult = null;
		reconfigActionId = '';
		try {
			const resp = await api.hubReconfigureGameServer(hubId, reconfigSlug, {
				port,
				net_ip: netIp,
				extra_args: extraArgs,
			});
			reconfigActionId = resp?.action_id || '';
			if (!reconfigActionId) throw new Error('Server did not return an action_id');
			await pollReconfigProgress();
		} catch (e) {
			reconfigError = e.message || 'Reconfigure failed';
			reconfigBusy = false;
		}
	}

	async function pollReconfigProgress() {
		if (!reconfigActionId) return;
		try {
			const data = await api.hubActionProgress(hubId, reconfigActionId);
			// Hub action progress events are `{step, message, percent}`; the
			// final result under `result.data.steps` uses `{step, ok, message}`.
			reconfigProgress = data?.events ?? [];
			if (data?.done) {
				reconfigResult = data.result || { ok: false, message: 'Unknown result' };
				reconfigBusy = false;
				if (reconfigResult?.ok) {
					await load();
				}
				return;
			}
		} catch (e) {
			console.warn('reconfigure progress poll failed', e);
		}
		reconfigPollTimer = setTimeout(pollReconfigProgress, 1000);
	}

	function fmtBytes(n) {
		if (!n) return '—';
		const units = ['B', 'KB', 'MB', 'GB', 'TB'];
		let i = 0; let v = Number(n);
		while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
		return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
	}

	function fmtAge(ts) {
		if (!ts) return 'never';
		const secs = Math.floor((Date.now() - new Date(ts).getTime()) / 1000);
		if (secs < 60) return `${secs}s ago`;
		if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
		if (secs < 86400) return `${Math.floor(secs / 3600)}h ago`;
		return `${Math.floor(secs / 86400)}d ago`;
	}

	function isOnline(h) {
		if (!h?.last_seen) return false;
		return (Date.now() - new Date(h.last_seen).getTime()) < 120_000;
	}

	function stateBadge(state) {
		const s = (state || '').toLowerCase();
		if (s === 'active' || s === 'running') return 'bg-green-500/10 text-green-400 border-green-500/20';
		if (s === 'activating' || s === 'reloading') return 'bg-yellow-500/10 text-yellow-400 border-yellow-500/20';
		if (s === 'failed') return 'bg-red-500/10 text-red-400 border-red-500/20';
		return 'bg-surface-800 text-surface-400 border-surface-700';
	}

	$effect(() => { load(); loadMetrics(); loadVersion(); });
	$effect(() => { if (metricsRange) loadMetrics(); });

	// Auto-refresh every 10s
	$effect(() => {
		const t = setInterval(() => { load(); loadMetrics(); }, 10_000);
		return () => clearInterval(t);
	});

	const latestMetric = $derived(metrics.length ? metrics[metrics.length - 1] : null);
</script>

<div class="mx-auto max-w-6xl space-y-6">
	<div class="flex items-center gap-3">
		<a href="/hubs" class="btn-ghost p-2" title="Back to hubs">
			<ArrowLeft class="h-4 w-4" />
		</a>
		<div class="flex-1">
			<div class="flex items-center gap-2">
				<HardDrive class="h-5 w-5 text-accent" />
				<h1 class="text-2xl font-bold text-surface-100">{hub?.name ?? `Hub ${hubId}`}</h1>
				{#if hub}
					{@const online = isOnline(hub)}
					<span class="flex items-center gap-1 text-xs">
						{#if online}
							<Wifi class="h-3 w-3 text-green-400" /><span class="text-green-400">online</span>
						{:else}
							<WifiOff class="h-3 w-3 text-surface-600" /><span class="text-surface-500">offline</span>
						{/if}
					</span>
				{/if}
			</div>
			<p class="mt-1 text-sm text-surface-500">
				{#if hostInfo?.hostname}{hostInfo.hostname} · {/if}
				{#if hostInfo?.os}{hostInfo.os}{/if}
				{#if hub?.last_seen} · last seen {fmtAge(hub.last_seen)}{/if}
			</p>
		</div>
		<button onclick={refresh} class="btn-secondary flex items-center gap-2" disabled={refreshing}>
			<RefreshCw class="h-4 w-4 {refreshing ? 'animate-spin' : ''}" />
			Refresh
		</button>
		<button onclick={restartHub} class="btn-secondary flex items-center gap-2" title="Restart hub process">
			<RotateCw class="h-4 w-4" />
			Restart hub
		</button>
	</div>

	{#if error}
		<div class="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">{error}</div>
	{/if}

	{#if loading}
		<div class="flex justify-center py-12">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
		</div>
	{:else}
		<!-- Host info cards -->
		<div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
			<div class="rounded-xl border border-surface-800 bg-surface-900 p-4">
				<div class="flex items-center gap-2 text-xs uppercase tracking-wide text-surface-500">
					<Cpu class="h-3 w-3" /> CPU
				</div>
				<div class="mt-2 text-xl font-semibold text-surface-100">
					{latestMetric?.cpu_pct != null ? `${latestMetric.cpu_pct.toFixed(1)}%` : '—'}
				</div>
				<div class="mt-1 text-xs text-surface-500">
					{hostInfo?.cpu_cores ?? '?'} cores · {hostInfo?.cpu_model ?? 'unknown'}
				</div>
			</div>
			<div class="rounded-xl border border-surface-800 bg-surface-900 p-4">
				<div class="flex items-center gap-2 text-xs uppercase tracking-wide text-surface-500">
					<MemoryStick class="h-3 w-3" /> Memory
				</div>
				<div class="mt-2 text-xl font-semibold text-surface-100">
					{latestMetric?.mem_pct != null ? `${latestMetric.mem_pct.toFixed(1)}%` : '—'}
				</div>
				<div class="mt-1 text-xs text-surface-500">
					of {fmtBytes(hostInfo?.total_ram_bytes)} total
				</div>
			</div>
			<div class="rounded-xl border border-surface-800 bg-surface-900 p-4">
				<div class="flex items-center gap-2 text-xs uppercase tracking-wide text-surface-500">
					<HardDriveDownload class="h-3 w-3" /> Disk
				</div>
				<div class="mt-2 text-xl font-semibold text-surface-100">
					{latestMetric?.disk_pct != null ? `${latestMetric.disk_pct.toFixed(1)}%` : '—'}
				</div>
				<div class="mt-1 text-xs text-surface-500">
					of {fmtBytes(hostInfo?.disk_total_bytes)} total
				</div>
			</div>
			<div class="rounded-xl border border-surface-800 bg-surface-900 p-4">
				<div class="text-xs uppercase tracking-wide text-surface-500">Load avg</div>
				<div class="mt-2 text-xl font-semibold text-surface-100">
					{latestMetric
						? `${latestMetric.load1?.toFixed(2) ?? '—'} ${latestMetric.load5?.toFixed(2) ?? ''} ${latestMetric.load15?.toFixed(2) ?? ''}`
						: '—'}
				</div>
				<div class="mt-1 text-xs text-surface-500">1m · 5m · 15m</div>
			</div>
		</div>

		<!-- Metrics sparkline -->
		<div class="rounded-xl border border-surface-800 bg-surface-900 p-4">
			<div class="flex items-center justify-between">
				<h2 class="text-sm font-semibold text-surface-200">Host metrics</h2>
				<div class="flex gap-1 text-xs">
					{#each ['1h', '6h', '24h', '7d'] as r}
						<button
							onclick={() => metricsRange = r}
							class="rounded px-2 py-1 {metricsRange === r ? 'bg-accent text-black' : 'bg-surface-800 text-surface-400 hover:bg-surface-700'}">
							{r}
						</button>
					{/each}
				</div>
			</div>
			{#if metrics.length === 0}
				<p class="mt-3 text-sm text-surface-500">No samples yet.</p>
			{:else}
				<div class="mt-3 grid gap-2">
					{#each [['cpu_pct', 'CPU %', 'text-accent'], ['mem_pct', 'Mem %', 'text-blue-400'], ['disk_pct', 'Disk %', 'text-purple-400']] as [key, label, cls]}
						{@const vals = metrics.map(m => m[key] ?? 0)}
						{@const max = Math.max(100, ...vals)}
						<div class="flex items-center gap-3">
							<span class="w-16 text-xs text-surface-500">{label}</span>
							<svg viewBox="0 0 {Math.max(vals.length, 2)} 100" preserveAspectRatio="none" class="h-10 flex-1 {cls}">
								<polyline
									fill="none"
									stroke="currentColor"
									stroke-width="1.5"
									vector-effect="non-scaling-stroke"
									points={vals.map((v, i) => `${i},${100 - (v / max) * 100}`).join(' ')} />
							</svg>
							<span class="w-12 text-right text-xs text-surface-400">{vals[vals.length - 1]?.toFixed(1) ?? '—'}</span>
						</div>
					{/each}
				</div>
			{/if}
		</div>

		<!-- Update card -->
		<div class="rounded-xl border border-surface-800 bg-surface-900 p-4">
			<div class="flex items-center justify-between">
				<h2 class="flex items-center gap-2 text-sm font-semibold text-surface-200">
					<Download class="h-4 w-4 text-accent" />
					Hub updates
				</h2>
				<button
					onclick={loadVersion}
					class="text-xs text-surface-500 hover:text-surface-200 flex items-center gap-1"
					disabled={versionLoading}>
					<RefreshCw class="h-3 w-3 {versionLoading ? 'animate-spin' : ''}" /> refresh
				</button>
			</div>

			{#if versionError}
				<div class="mt-3 rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-2 text-xs text-red-400">{versionError}</div>
			{/if}

			{#if versionInfo}
				{@const cached = versionInfo.cached}
				{@const latest = versionInfo.latest}
				{@const cachedBuild = cached?.build_hash ?? versionInfo.db_build_hash ?? null}
				{@const latestBuild = latest?.ok && latest?.build_hash ? latest.build_hash : null}
				{@const upToDate = latest?.up_to_date || (cachedBuild && latestBuild && cachedBuild === latestBuild)}

				<div class="mt-3 grid gap-3 sm:grid-cols-2">
					<div class="rounded-lg border border-surface-800 bg-surface-950 p-3">
						<div class="text-xs uppercase tracking-wide text-surface-500">Current build</div>
						<div class="mt-1 font-mono text-sm text-surface-100">{cachedBuild ?? 'unknown'}</div>
						{#if cached?.version}
							<div class="text-xs text-surface-500">v{cached.version}</div>
						{/if}
						{#if cached?.reported_at}
							<div class="mt-1 text-xs text-surface-500">reported {fmtAge(cached.reported_at)}</div>
						{/if}
					</div>
					<div class="rounded-lg border border-surface-800 bg-surface-950 p-3">
						<div class="text-xs uppercase tracking-wide text-surface-500">Latest available ({versionInfo.channel})</div>
						{#if latest?.ok === false}
							<div class="mt-1 text-sm text-red-400">{latest.error}</div>
						{:else if latest?.up_to_date}
							<div class="mt-1 text-sm text-surface-300">already latest</div>
						{:else if latest?.build_hash}
							<div class="mt-1 font-mono text-sm text-surface-100">{latest.build_hash}</div>
							<div class="text-xs text-surface-500">v{latest.version}</div>
							{#if latest.download_size}
								<div class="mt-1 text-xs text-surface-500">{fmtBytes(latest.download_size)}</div>
							{/if}
						{:else}
							<div class="mt-1 text-sm text-surface-500">—</div>
						{/if}
					</div>
				</div>

				<div class="mt-3 flex flex-wrap items-center gap-3">
					{#if upToDate}
						<span class="inline-flex items-center gap-1 rounded-full bg-green-500/10 border border-green-500/20 px-3 py-1 text-xs text-green-400">
							<CheckCircle2 class="h-3 w-3" /> Up to date
						</span>
					{:else if latestBuild}
						<span class="inline-flex items-center gap-1 rounded-full bg-yellow-500/10 border border-yellow-500/20 px-3 py-1 text-xs text-yellow-400">
							<AlertCircle class="h-3 w-3" /> Update available
						</span>
					{/if}

					<button
						onclick={forceUpdate}
						disabled={forceUpdating || !isOnline(hub)}
						class="btn-primary flex items-center gap-2 text-sm disabled:opacity-50">
						<Download class="h-4 w-4 {forceUpdating ? 'animate-pulse' : ''}" />
						{forceUpdating ? 'Triggering...' : 'Force update'}
					</button>

					<div class="flex items-center gap-2">
						<label class="text-xs text-surface-500">Release channel</label>
						<select
							bind:value={channelValue}
							class="rounded-md border border-surface-700 bg-surface-950 px-2 py-1 text-sm text-surface-100">
							{#each CHANNELS as c}
								<option value={c}>{c}</option>
							{/each}
						</select>
						<button
							onclick={changeUpdateChannel}
							disabled={channelSaving || channelValue === versionInfo.channel}
							class="btn-secondary text-sm disabled:opacity-50">
							{channelSaving ? 'Saving...' : 'Save'}
						</button>
					</div>

					<div class="flex items-center gap-2">
						<label class="text-xs text-surface-500">Check interval (s)</label>
						<input
							type="number"
							min="60"
							max="604800"
							bind:value={intervalDraft}
							class="w-28 rounded-md border border-surface-700 bg-surface-950 px-2 py-1 text-sm text-surface-100" />
						<button
							onclick={saveUpdateInterval}
							disabled={intervalSaving || String(versionInfo.update_interval ?? '') === String(intervalDraft)}
							class="btn-secondary text-sm disabled:opacity-50">
							{intervalSaving ? 'Saving...' : 'Save'}
						</button>
					</div>

					<div class="flex items-center gap-2">
						<label class="text-xs text-surface-500">Auto-update</label>
						<button
							onclick={toggleUpdateEnabled}
							disabled={updateEnabledSaving}
							class="btn-secondary text-sm disabled:opacity-50"
							title="Toggle whether this hub checks for and applies new builds automatically">
							{updateEnabledSaving ? 'Saving...' : (updateEnabled ? 'Enabled' : 'Disabled')}
						</button>
					</div>
				</div>

				{#if forceUpdateResult}
					<div class="mt-3 rounded-lg border px-3 py-2 text-xs {forceUpdateResult.ok === false ? 'border-red-500/20 bg-red-500/10 text-red-400' : 'border-green-500/20 bg-green-500/10 text-green-400'}">
						{forceUpdateResult.error ?? forceUpdateResult.message ?? JSON.stringify(forceUpdateResult)}
					</div>
				{/if}
				{#if channelResult}
					<div class="mt-2 rounded-lg border px-3 py-2 text-xs {channelResult.ok === false ? 'border-red-500/20 bg-red-500/10 text-red-400' : 'border-green-500/20 bg-green-500/10 text-green-400'}">
						{channelResult.error ?? channelResult.message ?? JSON.stringify(channelResult)}
					</div>
				{/if}
				{#if intervalResult}
					<div class="mt-2 rounded-lg border px-3 py-2 text-xs {intervalResult.ok === false ? 'border-red-500/20 bg-red-500/10 text-red-400' : 'border-green-500/20 bg-green-500/10 text-green-400'}">
						{intervalResult.error ?? intervalResult.message ?? JSON.stringify(intervalResult)}
					</div>
				{/if}
				{#if updateEnabledResult}
					<div class="mt-2 rounded-lg border px-3 py-2 text-xs {updateEnabledResult.ok === false ? 'border-red-500/20 bg-red-500/10 text-red-400' : 'border-green-500/20 bg-green-500/10 text-green-400'}">
						{updateEnabledResult.error ?? updateEnabledResult.message ?? JSON.stringify(updateEnabledResult)}
					</div>
				{/if}
			{:else if !versionLoading}
				<p class="mt-3 text-sm text-surface-500">No version info yet.</p>
			{/if}
		</div>

		<!-- Tabs -->
		<div class="border-b border-surface-800">
			<div class="flex gap-4">
				{#each [['clients', 'Clients'], ['host', 'Host info']] as [id, label]}
					<button
						onclick={() => tab = id}
						class="px-3 py-2 text-sm border-b-2 -mb-px transition-colors {tab === id ? 'border-accent text-surface-100' : 'border-transparent text-surface-500 hover:text-surface-300'}">
						{label}
					</button>
				{/each}
			</div>
		</div>

		{#if tab === 'clients'}
			<div class="space-y-3">
				<div class="flex items-center justify-between">
					<h2 class="text-sm font-semibold text-surface-200">Managed clients ({clients.length})</h2>
					<button onclick={() => showInstall = !showInstall} class="btn-secondary flex items-center gap-2 text-sm">
						<Plus class="h-4 w-4" />
						Install client
					</button>
				</div>

				{#if showInstall}
					<div class="rounded-xl border border-surface-800 bg-surface-900 p-4">
						{#if installActionId}
							<h3 class="text-sm font-semibold text-surface-200">
								{installResult ? (installResult.ok ? 'Install complete' : 'Install failed') : 'Installing client…'}
							</h3>
							<div class="mt-3">
								<div class="h-2 w-full overflow-hidden rounded bg-surface-800">
									<div
										class="h-full transition-all duration-300 {installResult && !installResult.ok ? 'bg-red-500' : 'bg-accent'}"
										style="width: {installPercent}%"
									></div>
								</div>
								<p class="mt-1 text-xs text-surface-500">{installPercent}%</p>
							</div>
							<ul class="mt-3 space-y-1.5 text-sm">
								{#each installProgress as ev}
									<li class="flex items-start gap-2 text-surface-300">
										<CheckCircle2 class="mt-0.5 h-4 w-4 shrink-0 text-emerald-400" />
										<span>
											<span class="font-mono text-xs text-surface-500">{ev.step}</span>
											<span class="ml-2">{ev.message}</span>
										</span>
									</li>
								{/each}
								{#if !installResult && installBusy}
									<li class="flex items-start gap-2 text-surface-400">
										<RefreshCw class="mt-0.5 h-4 w-4 shrink-0 animate-spin text-accent" />
										<span>Waiting for hub…</span>
									</li>
								{/if}
							</ul>
							{#if installResult}
								<div class="mt-3 rounded border px-3 py-2 text-sm {installResult.ok ? 'border-emerald-700 bg-emerald-900/20 text-emerald-200' : 'border-red-700 bg-red-900/20 text-red-200'}">
									{installResult.message}
								</div>
								<div class="mt-3 flex justify-end">
									<button onclick={dismissInstallProgress} class="btn-primary text-sm">Close</button>
								</div>
							{/if}
						{:else}
						<h3 class="text-sm font-semibold text-surface-200">Install new client</h3>
						<div class="mt-3 grid gap-3 sm:grid-cols-2">
							<label class="block">
								<span class="text-xs text-surface-500">Slug</span>
								<input
									type="text"
									bind:value={installSlug}
									placeholder="my-server"
									class="mt-1 w-full rounded bg-surface-800 border border-surface-700 px-3 py-2 text-sm text-surface-100 focus:border-accent focus:outline-none" />
							</label>
							<label class="block">
								<span class="text-xs text-surface-500">Server name</span>
								<input
									type="text"
									bind:value={installServerName}
									placeholder="My Server"
									class="mt-1 w-full rounded bg-surface-800 border border-surface-700 px-3 py-2 text-sm text-surface-100 focus:border-accent focus:outline-none" />
							</label>
						</div>

						<div class="mt-4 rounded-lg border border-surface-800 bg-surface-950 p-3">
							<label class="flex items-start gap-2 text-sm text-surface-200 cursor-pointer">
								<input
									type="checkbox"
									bind:checked={installGameServer}
									class="mt-0.5 h-4 w-4 rounded border-surface-600 bg-surface-800 text-accent focus:ring-accent" />
								<span>
									<span class="font-medium">Also install Urban Terror game server</span>
									<span class="block text-xs text-surface-500">
										When off, only the R3 client is installed — you can set up the game server from the
										server's detail page later.
									</span>
								</span>
							</label>

							{#if installGameServer}
								<div class="mt-3 grid gap-3 sm:grid-cols-2">
									<label class="block">
										<span class="text-xs text-surface-500">Max clients</span>
										<input
											type="number"
											min="2"
											max="64"
											bind:value={gsMaxClients}
											class="mt-1 w-full rounded bg-surface-800 border border-surface-700 px-3 py-2 font-mono text-sm text-surface-100 focus:border-accent focus:outline-none" />
									</label>
									<label class="block">
										<span class="text-xs text-surface-500">Game mode</span>
										<select
											bind:value={gsGameMode}
											class="mt-1 w-full rounded bg-surface-800 border border-surface-700 px-3 py-2 text-sm text-surface-100 focus:border-accent focus:outline-none">
											{#each GAME_MODES as m}
												<option value={m.value}>{m.label}</option>
											{/each}
										</select>
									</label>
								</div>

								<p class="mt-3 text-xs text-surface-500">
									Install path, public IP, and port are chosen automatically. RCON and referee passwords
									are generated as short random strings — you can view or rotate them later from the
									server detail page.
								</p>

								<button
									type="button"
									onclick={() => showAdvanced = !showAdvanced}
									class="mt-2 text-xs text-accent hover:text-accent/80">
									{showAdvanced ? '▾ Hide advanced options' : '▸ Advanced options'}
								</button>

								{#if showAdvanced}
									<div class="mt-3 grid gap-3 sm:grid-cols-2 rounded-lg border border-surface-800 bg-surface-900 p-3">
										<label class="block sm:col-span-2">
											<span class="text-xs text-surface-500">Install path</span>
											<input
												type="text"
												bind:value={gsInstallPath}
												placeholder="/home/urt/urt-{installSlug || 'my-server'}"
												class="mt-1 w-full rounded bg-surface-800 border border-surface-700 px-3 py-2 font-mono text-sm text-surface-100 focus:border-accent focus:outline-none" />
										</label>
										<label class="block">
											<span class="text-xs text-surface-500">Public IP</span>
											<input
												type="text"
												bind:value={gsPublicIp}
												placeholder="auto if blank"
												class="mt-1 w-full rounded bg-surface-800 border border-surface-700 px-3 py-2 font-mono text-sm text-surface-100 focus:border-accent focus:outline-none" />
										</label>
										<label class="block">
											<span class="text-xs text-surface-500">Port</span>
											<input
												type="number"
												min="1"
												max="65535"
												bind:value={gsPort}
												class="mt-1 w-full rounded bg-surface-800 border border-surface-700 px-3 py-2 font-mono text-sm text-surface-100 focus:border-accent focus:outline-none" />
										</label>
										<label class="flex items-center gap-2 text-sm text-surface-300 sm:col-span-2">
											<input
												type="checkbox"
												bind:checked={gsForceDownload}
												class="h-4 w-4 rounded border-surface-600 bg-surface-800 text-accent focus:ring-accent" />
											Force re-download UrT files
										</label>
									</div>
								{/if}
							{/if}
						</div>

						{#if installError}
							<p class="mt-2 text-sm text-red-400">{installError}</p>
						{/if}
						<div class="mt-3 flex justify-end gap-2">
							<button onclick={() => showInstall = false} class="btn-secondary text-sm" disabled={installBusy}>Cancel</button>
							<button onclick={submitInstall} class="btn-primary text-sm" disabled={installBusy}>
								{installBusy ? 'Installing…' : (installGameServer ? 'Install client + game server' : 'Install client only')}
							</button>
						</div>
						{/if}
					</div>
				{/if}

				{#if clients.length === 0}
					<div class="rounded-xl border border-surface-800 bg-surface-900 p-8 text-center text-sm text-surface-500">
						No clients installed on this hub yet.
					</div>
				{:else}
					<div class="overflow-hidden rounded-xl border border-surface-800 bg-surface-900">
						<table class="w-full text-sm">
							<thead class="bg-surface-800/50 text-left text-xs uppercase tracking-wide text-surface-500">
								<tr>
									<th class="px-4 py-3">Slug</th>
									<th class="px-4 py-3">State</th>
									<th class="px-4 py-3">PID</th>
									<th class="px-4 py-3">RSS</th>
									<th class="px-4 py-3 text-right">Actions</th>
								</tr>
							</thead>
							<tbody class="divide-y divide-surface-800">
								{#each clients as c (c.slug)}
									<tr>
										<td class="px-4 py-3 font-medium text-surface-100">{c.slug}</td>
										<td class="px-4 py-3">
											<span class="inline-flex rounded border px-2 py-0.5 text-xs {stateBadge(c.systemd_state)}">
												{c.systemd_state || 'unknown'}
											</span>
										</td>
										<td class="px-4 py-3 text-surface-400">{c.pid ?? '—'}</td>
										<td class="px-4 py-3 text-surface-400">{fmtBytes(c.rss_bytes)}</td>
										<td class="px-4 py-3">
											<div class="flex justify-end gap-1">
												<button
													onclick={() => clientAction(c.slug, 'start')}
													disabled={busySlug.startsWith(c.slug + ':')}
													class="btn-ghost p-1.5 text-green-400 hover:bg-green-500/10"
													title="Start">
													<Play class="h-4 w-4" />
												</button>
												<button
													onclick={() => clientAction(c.slug, 'stop')}
													disabled={busySlug.startsWith(c.slug + ':')}
													class="btn-ghost p-1.5 text-yellow-400 hover:bg-yellow-500/10"
													title="Stop">
													<Square class="h-4 w-4" />
												</button>
												<button
													onclick={() => clientAction(c.slug, 'restart')}
													disabled={busySlug.startsWith(c.slug + ':')}
													class="btn-ghost p-1.5 text-blue-400 hover:bg-blue-500/10"
													title="Restart">
													<RotateCw class="h-4 w-4" />
												</button>
												{#if c.server_id}
													<a href={`/servers/${c.server_id}`} class="btn-ghost p-1.5" title="Open server">
														<Terminal class="h-4 w-4" />
													</a>
												{/if}
												<button
													onclick={() => openReconfig(c)}
													disabled={busySlug.startsWith(c.slug + ':')}
													class="btn-ghost p-1.5 text-purple-400 hover:bg-purple-500/10"
													title="Configure game server (port, bind IP, extra ExecStart args)">
													<Sliders class="h-4 w-4" />
												</button>
												<button
													onclick={() => uninstallClient(c.slug)}
													disabled={busySlug.startsWith(c.slug + ':')}
													class="btn-ghost p-1.5 text-red-400 hover:bg-red-500/10"
													title="Uninstall">
													<Trash2 class="h-4 w-4" />
												</button>
											</div>
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{/if}
			</div>
		{:else if tab === 'host'}
			<div class="rounded-xl border border-surface-800 bg-surface-900 p-5">
				{#if !hostInfo}
					<p class="text-sm text-surface-500">No host info reported yet.</p>
				{:else}
					<dl class="grid gap-3 sm:grid-cols-2 text-sm">
						<div><dt class="text-xs uppercase text-surface-500">Hostname</dt><dd class="text-surface-200">{hostInfo.hostname || '—'}</dd></div>
						<div><dt class="text-xs uppercase text-surface-500">OS</dt><dd class="text-surface-200">{hostInfo.os || '—'}</dd></div>
						<div><dt class="text-xs uppercase text-surface-500">Kernel</dt><dd class="text-surface-200">{hostInfo.kernel || '—'}</dd></div>
						<div><dt class="text-xs uppercase text-surface-500">CPU</dt><dd class="text-surface-200">{hostInfo.cpu_model || '—'} ({hostInfo.cpu_cores ?? '?'} cores)</dd></div>
						<div><dt class="text-xs uppercase text-surface-500">RAM</dt><dd class="text-surface-200">{fmtBytes(hostInfo.total_ram_bytes)}</dd></div>
						<div><dt class="text-xs uppercase text-surface-500">Disk</dt><dd class="text-surface-200">{fmtBytes(hostInfo.disk_total_bytes)}</dd></div>
						<div><dt class="text-xs uppercase text-surface-500">Public IP</dt><dd class="text-surface-200">{hostInfo.public_ip || '—'}</dd></div>
						<div><dt class="text-xs uppercase text-surface-500">External IP</dt><dd class="text-surface-200">{hostInfo.external_ip || '—'}</dd></div>
					</dl>
					{#if hostInfo.urt_installs_json}
						<div class="mt-4">
							<div class="text-xs uppercase text-surface-500">UrT installs</div>
							<pre class="mt-1 overflow-x-auto rounded bg-surface-950 p-3 text-xs text-surface-300">{hostInfo.urt_installs_json}</pre>
						</div>
					{/if}
				{/if}
			</div>
		{/if}
	{/if}
</div>

{#if showReconfig}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4" role="dialog" aria-modal="true">
		<div class="w-full max-w-lg rounded-xl border border-surface-800 bg-surface-900 p-5 shadow-xl">
			<div class="mb-3 flex items-center justify-between">
				<h3 class="text-lg font-semibold text-surface-100">Configure game server</h3>
				<button onclick={dismissReconfig} class="btn-ghost text-sm" disabled={reconfigBusy}>Close</button>
			</div>
			<p class="mb-3 text-xs text-surface-400">
				Rewrites the <code class="text-surface-200">urt@{reconfigSlug}.service</code> drop-in with new
				start-time options and restarts the unit. Runtime settings (maps, rcon, plugins)
				belong on the Servers page instead.
			</p>

			{#if !reconfigResult}
				<div class="space-y-3">
					<label class="block text-sm">
						<span class="mb-1 block text-xs uppercase text-surface-400">Port</span>
						<input type="number" min="1" max="65535" bind:value={reconfigPort} disabled={reconfigBusy}
							class="input w-full" />
					</label>
					<label class="block text-sm">
						<span class="mb-1 block text-xs uppercase text-surface-400">Bind IP <span class="text-surface-600">(optional)</span></span>
						<input type="text" bind:value={reconfigNetIp} disabled={reconfigBusy}
							placeholder="blank = bind all interfaces"
							class="input w-full" />
					</label>
					<label class="block text-sm">
						<span class="mb-1 block text-xs uppercase text-surface-400">Extra ExecStart args <span class="text-surface-600">(optional)</span></span>
						<input type="text" bind:value={reconfigExtraArgsText} disabled={reconfigBusy}
							placeholder="+set com_hunkmegs 512 +exec extra.cfg"
							class="input w-full font-mono text-xs" />
						<span class="mt-1 block text-[11px] text-surface-500">
							Whitespace-separated. No shell metacharacters; quoted values aren't supported.
						</span>
					</label>
					{#if reconfigError}
						<div class="rounded border border-red-500/40 bg-red-500/10 p-2 text-xs text-red-300">{reconfigError}</div>
					{/if}
					<div class="flex justify-end gap-2 pt-2">
						<button onclick={dismissReconfig} disabled={reconfigBusy} class="btn-ghost">Cancel</button>
						<button onclick={submitReconfig} disabled={reconfigBusy} class="btn-primary">
							{reconfigBusy ? 'Applying…' : 'Apply & restart'}
						</button>
					</div>
				</div>
			{/if}

			{#if reconfigActionId}
				<div class="mt-4 space-y-1 text-xs">
					{#each reconfigProgress as ev}
						<div class="flex items-start gap-2 text-surface-300">
							<span class="text-surface-500">{ev.step}</span>
							<span class="text-surface-400">{ev.message ?? ''}</span>
							{#if typeof ev.percent === 'number'}
								<span class="ml-auto text-surface-500">{ev.percent}%</span>
							{/if}
						</div>
					{/each}
				</div>
			{/if}

			{#if reconfigResult}
				<div class="mt-3 rounded border p-3 text-sm"
					class:border-green-500={reconfigResult.ok}
					class:bg-green-500={reconfigResult.ok}
					class:border-red-500={!reconfigResult.ok}
					class:bg-red-500={!reconfigResult.ok}
					class:border-opacity-40={true}
					class:bg-opacity-10={true}>
					<div class="flex items-center gap-2">
						{#if reconfigResult.ok}
							<CheckCircle2 class="h-4 w-4 text-green-400" />
							<span class="text-green-300">Applied. Unit restarted on port {reconfigPort}.</span>
						{:else}
							<AlertCircle class="h-4 w-4 text-red-400" />
							<span class="text-red-300">{reconfigResult.message || 'Reconfigure failed.'}</span>
						{/if}
					</div>
					{#if reconfigResult.data?.steps}
						<ul class="mt-2 space-y-0.5 text-xs text-surface-300">
							{#each reconfigResult.data.steps as s}
								<li class:text-green-400={s.ok} class:text-red-400={!s.ok}>
									[{s.ok ? 'ok' : 'fail'}] {s.step}{s.message ? ' — ' + s.message : ''}
								</li>
							{/each}
						</ul>
					{/if}
					<div class="mt-3 flex justify-end">
						<button onclick={dismissReconfig} class="btn-primary">Close</button>
					</div>
				</div>
			{/if}
		</div>
	</div>
{/if}
