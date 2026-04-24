<script>
	import { page } from '$app/stores';
	import { api } from '$lib/api.svelte.js';
	import {
		HardDrive, RefreshCw, ArrowLeft, Cpu, MemoryStick, HardDriveDownload,
		Play, Square, RotateCw, Trash2, Plus, Terminal, Wifi, WifiOff,
		Download, CheckCircle2, AlertCircle
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

	// Per-client action busy flags
	let busySlug = $state('');

	// Update card state
	let versionInfo = $state(null);
	let versionLoading = $state(false);
	let versionError = $state('');
	let forceUpdating = $state(false);
	let forceUpdateResult = $state(null);
	let channelSaving = $state(false);
	let channelResult = $state(null);
	let channelValue = $state('beta');

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
		if (!confirm(`Uninstall client "${slug}"? This removes its data from the hub.`)) return;
		busySlug = slug + ':uninstall';
		try {
			await api.hubUninstallClient(hubId, slug, true);
			await load();
		} catch (e) {
			error = e.message || `Failed to uninstall ${slug}`;
		}
		busySlug = '';
	}

	async function submitInstall() {
		installError = '';
		if (!installSlug.trim()) { installError = 'Slug is required'; return; }
		installBusy = true;
		try {
			await api.hubInstallClient(hubId, {
				slug: installSlug.trim(),
				server_name: installServerName.trim() || installSlug.trim(),
				register_systemd: true
			});
			installSlug = '';
			installServerName = '';
			showInstall = false;
			await load();
		} catch (e) {
			installError = e.message || 'Install failed';
		}
		installBusy = false;
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
						{#if installError}
							<p class="mt-2 text-sm text-red-400">{installError}</p>
						{/if}
						<div class="mt-3 flex justify-end gap-2">
							<button onclick={() => showInstall = false} class="btn-secondary text-sm" disabled={installBusy}>Cancel</button>
							<button onclick={submitInstall} class="btn-primary text-sm" disabled={installBusy}>
								{installBusy ? 'Installing…' : 'Install'}
							</button>
						</div>
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
