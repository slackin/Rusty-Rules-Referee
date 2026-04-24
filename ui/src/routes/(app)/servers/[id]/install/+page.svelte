<script>
	import { api } from '$lib/api.svelte.js';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import {
		ArrowLeft, ArrowRight, CheckCircle2, XCircle, Loader2, Search,
		ServerCog, FolderDown, Network, FileCog, Shield, Rocket, RefreshCw
	} from 'lucide-svelte';

	const serverId = $derived(Number($page.params.id));

	// Wizard state
	let step = $state(0); // 0=welcome, 1=path, 2=ports, 3=basics, 4=systemd, 5=review, 6=installing, 7=complete
	let loading = $state(true);
	let error = $state('');

	// Defaults from the client
	let suggested = $state(null); // response from wizardSuggest
	let existingState = $state(null);

	// Form values
	let installPath = $state('');
	let port = $state(27960);
	let slug = $state('');
	let hostname = $state('');
	let serverName = $state('');
	let publicIp = $state('');
	let rconPassword = $state('');
	let adminPassword = $state('');
	let gameMode = $state('CTF');
	let maxClients = $state(16);
	let registerSystemd = $state(true);
	let forceDownload = $state(false);

	// Port probe state
	let probing = $state(false);
	let portProbe = $state(null);

	// Install state
	let installStatus = $state(null);
	let installErr = $state('');
	let pollTimer = null;

	const GAME_MODES = [
		{ value: 'FFA', label: 'Free for All (g_gametype 0)' },
		{ value: 'LMS', label: 'Last Man Standing (1)' },
		{ value: 'TDM', label: 'Team Deathmatch (3)' },
		{ value: 'TS', label: 'Team Survivor (4)' },
		{ value: 'FTL', label: 'Follow the Leader (5)' },
		{ value: 'CAH', label: 'Capture and Hold (6)' },
		{ value: 'CTF', label: 'Capture the Flag (7)' },
		{ value: 'BOMB', label: 'Bomb (8)' },
		{ value: 'JUMP', label: 'Jump (9)' },
		{ value: 'FREEZE', label: 'Freeze Tag (10)' },
		{ value: 'GUNGAME', label: 'Gun Game (11)' },
	];

	$effect(() => {
		loadSuggestions();
		return () => {
			if (pollTimer) clearInterval(pollTimer);
		};
	});

	async function loadSuggestions() {
		loading = true;
		error = '';
		try {
			const r = await api.wizardSuggest(serverId);
			// Response may be flat or wrapped in { response_type, data }
			const d = r?.data ?? r;
			suggested = d;
			existingState = d?.state ?? null;
			installPath = d?.suggested_install_path || '';
			port = d?.suggested_port || 27960;
			slug = d?.suggested_slug || '';
			serverName = d?.suggested_server_name || '';
			hostname = d?.suggested_server_name || '';
			if (existingState?.configured) {
				step = 5; // jump to review (read-only); force_download path still works
			}
		} catch (e) {
			error = String(e?.message || e);
		} finally {
			loading = false;
		}
	}

	async function probePort() {
		probing = true;
		portProbe = null;
		try {
			const r = await api.wizardProbePorts(serverId, [Number(port)], 'udp');
			portProbe = r?.data ?? r;
		} catch (e) {
			portProbe = { error: String(e?.message || e) };
		} finally {
			probing = false;
		}
	}

	function next() {
		if (step < 5) step++;
		else if (step === 5) launchInstall();
	}
	function back() { if (step > 0) step--; }

	async function launchInstall() {
		installErr = '';
		installStatus = null;
		step = 6;
		try {
			const params = {
				install_path: installPath,
				hostname: hostname || serverName,
				public_ip: publicIp || '',
				port: Number(port),
				rcon_password: rconPassword,
				game_mode: gameMode,
				max_clients: Number(maxClients),
				admin_password: adminPassword || null,
				register_systemd: registerSystemd,
				slug: slug || null,
				force_download: forceDownload
			};
			await api.wizardInstall(serverId, params);
			// Poll install-status
			pollTimer = setInterval(pollInstall, 1500);
		} catch (e) {
			installErr = String(e?.message || e);
			step = 5;
		}
	}

	async function pollInstall() {
		try {
			const r = await api.installStatus(serverId);
			const d = r?.data ?? r;
			installStatus = d;
			if (d?.stage === 'complete' || d?.completed) {
				clearInterval(pollTimer);
				pollTimer = null;
				step = 7;
			} else if (d?.stage === 'error') {
				clearInterval(pollTimer);
				pollTimer = null;
				installErr = d?.error || 'Install failed';
				step = 5;
			}
		} catch (e) {
			installErr = String(e?.message || e);
		}
	}

	function canNext() {
		switch (step) {
			case 0: return !loading && !error;
			case 1: return installPath.trim().length > 0;
			case 2: return portProbe?.results?.[0]?.available === true || portProbe?.results?.[0]?.bind_succeeded === true;
			case 3: return serverName.trim() && rconPassword.trim() && maxClients >= 2 && maxClients <= 64;
			case 4: return true;
			case 5: return true;
			default: return false;
		}
	}
</script>

<div class="max-w-4xl mx-auto px-4 py-6 space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-2xl font-bold flex items-center gap-2"><Rocket class="h-6 w-6 text-accent" /> Install Game Server</h1>
			<p class="text-sm text-surface-500">Download Urban Terror 4.3, configure it, and (optionally) register a managed systemd service — all driven from this UI.</p>
		</div>
		<a href={`/servers/${serverId}`} class="text-sm text-surface-400 hover:text-surface-100 flex items-center gap-1"><ArrowLeft class="h-4 w-4" /> Back to server</a>
	</div>

	<!-- Progress -->
	<ol class="flex flex-wrap gap-2 text-xs">
		{#each ['Welcome','Install path','Port','Basics','Systemd','Review','Install','Done'] as label, i}
			<li class="px-2 py-1 rounded-full border {step === i ? 'border-accent text-accent' : step > i ? 'border-emerald-500/40 text-emerald-400' : 'border-surface-700 text-surface-500'}">
				{i+1}. {label}
			</li>
		{/each}
	</ol>

	{#if loading}
		<div class="rounded-xl border border-surface-800 bg-surface-900 p-8 text-center">
			<Loader2 class="h-5 w-5 animate-spin inline mr-2" /> Loading client defaults…
		</div>
	{:else if error}
		<div class="rounded-xl border border-red-500/40 bg-red-500/5 p-6 space-y-2">
			<div class="flex items-center gap-2 text-red-400 font-semibold"><XCircle class="h-4 w-4" /> {error}</div>
			<p class="text-sm text-surface-400">The client bot may be offline. Make sure it's connected and try again.</p>
			<button onclick={loadSuggestions} class="mt-2 px-3 py-1.5 rounded-md border border-surface-700 text-sm hover:bg-surface-800"><RefreshCw class="h-3.5 w-3.5 inline mr-1" /> Retry</button>
		</div>
	{:else}
		<!-- Existing-install banner -->
		{#if existingState?.configured}
			<div class="rounded-xl border border-amber-500/40 bg-amber-500/5 p-4 text-sm">
				<div class="flex items-center gap-2 text-amber-400 font-semibold"><CheckCircle2 class="h-4 w-4" /> A game server is already configured on this client.</div>
				<p class="mt-1 text-surface-400">
					Install path: <code class="text-surface-200">{existingState.install_path}</code> ·
					Port: <code class="text-surface-200">{existingState.port}</code>
					{#if existingState.service_name} · Service: <code class="text-surface-200">{existingState.service_name}</code>{/if}
				</p>
				<p class="mt-2 text-xs text-surface-500">Re-running the wizard will require "force" and may overwrite files. Manage the service below instead.</p>
				{#if existingState.service_name}
					<div class="mt-3 flex gap-2">
						<button class="px-3 py-1.5 rounded-md border border-surface-700 text-xs hover:bg-surface-800" onclick={async ()=>{ try { await api.wizardServiceAction(serverId, 'start'); } catch(e){} }}>Start</button>
						<button class="px-3 py-1.5 rounded-md border border-surface-700 text-xs hover:bg-surface-800" onclick={async ()=>{ try { await api.wizardServiceAction(serverId, 'stop'); } catch(e){} }}>Stop</button>
						<button class="px-3 py-1.5 rounded-md border border-surface-700 text-xs hover:bg-surface-800" onclick={async ()=>{ try { await api.wizardServiceAction(serverId, 'restart'); } catch(e){} }}>Restart</button>
						<button class="px-3 py-1.5 rounded-md border border-surface-700 text-xs hover:bg-surface-800" onclick={async ()=>{ try { await api.wizardServiceAction(serverId, 'status'); } catch(e){} }}>Status</button>
					</div>
				{/if}
				<label class="mt-3 flex items-center gap-2 text-xs cursor-pointer">
					<input type="checkbox" bind:checked={forceDownload} /> Force re-install (overwrites existing config)
				</label>
			</div>
		{/if}

		<!-- Step content -->
		<div class="rounded-xl border border-surface-800 bg-surface-900 p-6 space-y-4">
			{#if step === 0}
				<h2 class="text-lg font-semibold flex items-center gap-2"><ServerCog class="h-5 w-5 text-accent" /> Welcome</h2>
				<p class="text-sm text-surface-400">This wizard will:</p>
				<ul class="text-sm space-y-1 list-disc list-inside text-surface-300">
					<li>Download Urban Terror 4.3 into a per-instance directory (or use an existing one)</li>
					<li>Probe the chosen UDP port to make sure it's free</li>
					<li>Generate a curated <code>server.cfg</code> with R3-recommended defaults</li>
					<li>Optionally register a managed <code>urt@{slug || '<slug>'}.service</code> systemd unit so you can start/stop it from the UI</li>
				</ul>
				<p class="text-xs text-surface-500">One game server per client bot. Multiple clients can coexist on one host — each gets its own slug, dir, port, and service.</p>
				{#if !suggested?.scaffolding_present}
					<div class="text-xs bg-amber-500/5 border border-amber-500/30 rounded-md p-3 text-amber-300">
						⚠ Systemd scaffolding is not installed on this client. You can still install files + config, but "Register systemd service" will be unavailable. Re-run <code>install-r3.sh --add-urt</code> on the client to enable it.
					</div>
				{/if}

			{:else if step === 1}
				<h2 class="text-lg font-semibold flex items-center gap-2"><FolderDown class="h-5 w-5 text-accent" /> Install path</h2>
				<p class="text-sm text-surface-400">Where should Urban Terror 4.3 live on the client? This needs to be writable by the bot's user account and unique to this client.</p>
				<label class="block text-xs font-medium text-surface-400">Install directory</label>
				<input type="text" bind:value={installPath} class="w-full rounded-md border border-surface-700 bg-surface-950 px-3 py-2 text-sm" />
				<label class="block text-xs font-medium text-surface-400 mt-3">Slug (used in paths and service name)</label>
				<input type="text" bind:value={slug} class="w-full rounded-md border border-surface-700 bg-surface-950 px-3 py-2 text-sm" />
				<p class="text-xs text-surface-500">The wizard suggests: <code>{suggested?.suggested_install_path}</code></p>

			{:else if step === 2}
				<h2 class="text-lg font-semibold flex items-center gap-2"><Network class="h-5 w-5 text-accent" /> UDP port</h2>
				<p class="text-sm text-surface-400">Urban Terror uses a UDP port for the game server. We check it two ways — by asking the kernel's bound-socket table (<code>ss</code>) and by trying to open the port ourselves — so port collisions between instances on the same host are caught now, not at launch.</p>
				<div class="flex items-center gap-2">
					<input type="number" min="1024" max="65535" bind:value={port} class="w-32 rounded-md border border-surface-700 bg-surface-950 px-3 py-2 text-sm" />
					<button onclick={probePort} disabled={probing} class="px-3 py-2 rounded-md border border-surface-700 text-sm hover:bg-surface-800 disabled:opacity-50">
						{#if probing}<Loader2 class="h-4 w-4 inline animate-spin" />{:else}<Search class="h-4 w-4 inline" />{/if}
						Check port
					</button>
				</div>
				{#if portProbe}
					{#if portProbe.error}
						<div class="text-xs text-red-400">{portProbe.error}</div>
					{:else if portProbe.results}
						{@const r = portProbe.results[0]}
						<div class="rounded-md border p-3 text-sm {r.available ? 'border-emerald-500/40 bg-emerald-500/5 text-emerald-300' : 'border-red-500/40 bg-red-500/5 text-red-300'}">
							{#if r.available}<CheckCircle2 class="h-4 w-4 inline" /> Port {r.port}/udp is available.{:else}<XCircle class="h-4 w-4 inline" /> Port {r.port}/udp is in use.{/if}
							<div class="mt-1 text-xs text-surface-400">{r.detail}</div>
						</div>
					{/if}
				{:else}
					<p class="text-xs text-surface-500">Suggested default: {suggested?.suggested_port}. Click "Check port" to verify.</p>
				{/if}

			{:else if step === 3}
				<h2 class="text-lg font-semibold flex items-center gap-2"><FileCog class="h-5 w-5 text-accent" /> Basics</h2>
				<div class="grid sm:grid-cols-2 gap-3 text-sm">
					<label class="space-y-1">
						<span class="block text-xs font-medium text-surface-400">Server name (sv_hostname)</span>
						<input type="text" bind:value={serverName} class="w-full rounded-md border border-surface-700 bg-surface-950 px-3 py-2" />
					</label>
					<label class="space-y-1">
						<span class="block text-xs font-medium text-surface-400">Public IP (optional)</span>
						<input type="text" bind:value={publicIp} placeholder="auto-detect" class="w-full rounded-md border border-surface-700 bg-surface-950 px-3 py-2" />
					</label>
					<label class="space-y-1">
						<span class="block text-xs font-medium text-surface-400">RCON password *</span>
						<input type="password" bind:value={rconPassword} class="w-full rounded-md border border-surface-700 bg-surface-950 px-3 py-2" />
					</label>
					<label class="space-y-1">
						<span class="block text-xs font-medium text-surface-400">Admin password (optional)</span>
						<input type="password" bind:value={adminPassword} class="w-full rounded-md border border-surface-700 bg-surface-950 px-3 py-2" />
					</label>
					<label class="space-y-1">
						<span class="block text-xs font-medium text-surface-400">Game mode</span>
						<select bind:value={gameMode} class="w-full rounded-md border border-surface-700 bg-surface-950 px-3 py-2">
							{#each GAME_MODES as m}<option value={m.value}>{m.label}</option>{/each}
						</select>
					</label>
					<label class="space-y-1">
						<span class="block text-xs font-medium text-surface-400">Max clients</span>
						<input type="number" min="2" max="64" bind:value={maxClients} class="w-full rounded-md border border-surface-700 bg-surface-950 px-3 py-2" />
					</label>
				</div>

			{:else if step === 4}
				<h2 class="text-lg font-semibold flex items-center gap-2"><Shield class="h-5 w-5 text-accent" /> Systemd management</h2>
				<p class="text-sm text-surface-400">If enabled, we'll register a per-instance drop-in under <code>urt@{slug}.service</code> so you can start/stop/restart the game server from this UI (no SSH required).</p>
				<label class="flex items-center gap-2 text-sm cursor-pointer {suggested?.scaffolding_present ? '' : 'opacity-50 cursor-not-allowed'}">
					<input type="checkbox" bind:checked={registerSystemd} disabled={!suggested?.scaffolding_present} />
					Register managed systemd service
				</label>
				{#if !suggested?.scaffolding_present}
					<p class="text-xs text-amber-400">Scaffolding is not installed on this client. Run <code>sudo bash install-r3.sh --add-urt</code> on the host to enable.</p>
				{/if}

			{:else if step === 5}
				<h2 class="text-lg font-semibold flex items-center gap-2"><CheckCircle2 class="h-5 w-5 text-accent" /> Review</h2>
				<dl class="text-sm grid sm:grid-cols-2 gap-x-6 gap-y-2">
					<dt class="text-surface-500">Install path</dt><dd class="text-surface-200"><code>{installPath}</code></dd>
					<dt class="text-surface-500">Slug</dt><dd class="text-surface-200">{slug}</dd>
					<dt class="text-surface-500">Port</dt><dd class="text-surface-200">{port}/udp</dd>
					<dt class="text-surface-500">Server name</dt><dd class="text-surface-200">{serverName}</dd>
					<dt class="text-surface-500">Public IP</dt><dd class="text-surface-200">{publicIp || '(auto)'}</dd>
					<dt class="text-surface-500">Game mode</dt><dd class="text-surface-200">{gameMode}</dd>
					<dt class="text-surface-500">Max clients</dt><dd class="text-surface-200">{maxClients}</dd>
					<dt class="text-surface-500">RCON password</dt><dd class="text-surface-200">{'•'.repeat(rconPassword.length)}</dd>
					<dt class="text-surface-500">Admin password</dt><dd class="text-surface-200">{adminPassword ? '•'.repeat(adminPassword.length) : '(none)'}</dd>
					<dt class="text-surface-500">Systemd service</dt><dd class="text-surface-200">{registerSystemd ? `urt@${slug}.service` : '(unmanaged)'}</dd>
					<dt class="text-surface-500">Force re-download</dt><dd class="text-surface-200">{forceDownload ? 'yes' : 'no'}</dd>
				</dl>
				{#if installErr}
					<div class="rounded-md border border-red-500/40 bg-red-500/5 p-3 text-sm text-red-300"><XCircle class="h-4 w-4 inline" /> {installErr}</div>
				{/if}

			{:else if step === 6}
				<h2 class="text-lg font-semibold flex items-center gap-2"><Loader2 class="h-5 w-5 animate-spin" /> Installing…</h2>
				{#if installStatus}
					<div class="text-sm text-surface-300">Stage: <span class="text-accent">{installStatus.stage}</span></div>
					<div class="w-full h-2 bg-surface-800 rounded overflow-hidden">
						<div class="h-full bg-accent transition-all" style="width: {installStatus.percent || 0}%"></div>
					</div>
					<div class="text-xs text-surface-500">{installStatus.percent || 0}%</div>
				{:else}
					<p class="text-sm text-surface-400">Kicking off install on the client…</p>
				{/if}

			{:else if step === 7}
				<h2 class="text-lg font-semibold flex items-center gap-2 text-emerald-400"><CheckCircle2 class="h-5 w-5" /> Install complete</h2>
				<div class="text-sm text-surface-300 space-y-1">
					<div>Install path: <code class="text-surface-200">{installStatus?.install_path || installPath}</code></div>
					{#if installStatus?.game_log}<div>Game log: <code class="text-surface-200">{installStatus.game_log}</code></div>{/if}
					{#if installStatus?.public_ip}<div>Public IP: <code class="text-surface-200">{installStatus.public_ip}</code></div>{/if}
					{#if installStatus?.port}<div>Port: <code class="text-surface-200">{installStatus.port}/udp</code></div>{/if}
					{#if installStatus?.service_name}<div>Service: <code class="text-surface-200">{installStatus.service_name}</code></div>{/if}
				</div>
				<div class="rounded-md border border-emerald-500/30 bg-emerald-500/5 p-3 text-xs text-emerald-300">
					<CheckCircle2 class="h-3.5 w-3.5 inline mr-1" />
					These values have been saved to the server's Manual Configuration and pushed to the client. No further setup is required — you can start the game server now.
				</div>
				<div class="flex gap-2">
					<a href={`/servers/${serverId}`} class="px-3 py-2 rounded-md border border-surface-700 text-sm hover:bg-surface-800">Back to server</a>
					<a href={`/servers/${serverId}/server-cfg`} class="px-3 py-2 rounded-md border border-surface-700 text-sm hover:bg-surface-800">Edit server.cfg</a>
				</div>
			{/if}
		</div>

		<!-- Navigation -->
		{#if step < 6}
			<div class="flex justify-between">
				<button onclick={back} disabled={step === 0} class="px-3 py-2 rounded-md border border-surface-700 text-sm hover:bg-surface-800 disabled:opacity-30 flex items-center gap-1"><ArrowLeft class="h-4 w-4" /> Back</button>
				<button onclick={next} disabled={!canNext()} class="px-4 py-2 rounded-md bg-accent text-white text-sm font-semibold hover:bg-accent/90 disabled:opacity-40 flex items-center gap-1">
					{#if step === 5}Start install <Rocket class="h-4 w-4" />{:else}Next <ArrowRight class="h-4 w-4" />{/if}
				</button>
			</div>
		{/if}
	{/if}
</div>
