<script>
	import { api } from '$lib/api.svelte.js';
	import { getServerStatus, getOnlinePlayers, getRecentEvents, getRecentChat, getRecentVotes, isInitialized } from '$lib/live.svelte.js';
	import { getAuth } from '$lib/auth.svelte.js';
	import { isMaster } from '$lib/mode.svelte.js';
	import { stripColors, timeAgo } from '$lib/utils.js';
	import { Users, Map, Clock, Activity, Zap, Shield, Ban, AlertTriangle, MessageSquare, Vote, StickyNote, ExternalLink, Search, ArrowRight, SkipForward, X, ChevronDown, ChevronUp, UserX, ShieldBan, Eye, Send, Server, Wifi, WifiOff, Link } from 'lucide-svelte';

	// Master dashboard state
	let masterServers = $state([]);
	let masterLoading = $state(true);
	let masterError = $state('');

	async function loadMasterData() {
		try {
			masterServers = await api.servers();
			masterError = '';
		} catch (e) {
			masterError = e.message || 'Failed to load servers';
		}
		masterLoading = false;
	}

	$effect(() => {
		if (isMaster()) {
			loadMasterData();
			const interval = setInterval(loadMasterData, 15000);
			return () => clearInterval(interval);
		}
	});

	let onlineCount = $derived(masterServers.filter(s => s.online).length);
	let totalPlayers = $derived(masterServers.reduce((sum, s) => sum + (s.online ? s.player_count : 0), 0));
	let totalCapacity = $derived(masterServers.reduce((sum, s) => sum + (s.online ? s.max_clients : 0), 0));

	let status = $derived(getServerStatus());
	let players = $derived(getOnlinePlayers());
	let recentEvents = $derived(getRecentEvents());
	let recentChat = $derived(getRecentChat());
	let recentVotes = $derived(getRecentVotes());
	let loading = $derived(!isInitialized());
	let auth = $derived(getAuth());
	let isAdmin = $derived(auth?.user?.role === 'admin');

	let summary = $state(null);
	let notes = $state('');
	let notesSaving = $state(false);
	let notesSaved = $state(false);

	// Map modal state
	let showMapModal = $state(false);
	let mapList = $state([]);
	let mapsLoading = $state(false);
	let mapSearch = $state('');
	let selectedMap = $state(null);
	let mapActionLoading = $state(false);
	let mapActionResult = $state(null);

	let filteredMaps = $derived(
		mapSearch
			? mapList.filter(m => m.toLowerCase().includes(mapSearch.toLowerCase()))
			: mapList
	);

	async function openMapModal() {
		if (!isAdmin) return;
		showMapModal = true;
		mapsLoading = true;
		mapSearch = '';
		selectedMap = null;
		mapActionResult = null;
		try {
			const res = await api.mapList();
			const list = Array.isArray(res?.maps) ? res.maps : [];
			mapList = list.map((m) => (typeof m === 'string' ? m : m.map_name)).filter(Boolean);
		} catch (e) {
			mapList = [];
			console.error('Failed to load maps:', e);
		}
		mapsLoading = false;
	}

	async function doMapAction(action) {
		if (!selectedMap) return;
		mapActionLoading = true;
		mapActionResult = null;
		try {
			const res = await api.changeMap(selectedMap, action);
			mapActionResult = { success: true, message: res.message };
			setTimeout(() => { showMapModal = false; }, 1500);
		} catch (e) {
			mapActionResult = { success: false, message: e.message || 'Failed' };
		}
		mapActionLoading = false;
	}

	const gameTypes = { '0': 'FFA', '1': 'LMS', '3': 'TDM', '4': 'TS', '5': 'FTL', '6': 'C&H', '7': 'CTF', '8': 'Bomb', '9': 'Jump', '10': 'Freeze', '11': 'GunGame' };

	// Load summary stats + personal notes
	$effect(() => {
		api.dashboardSummary().then(s => { summary = s; }).catch(() => {});
		api.notes().then(n => { notes = n ?? ''; }).catch(() => {});
	});

	let statCards = $derived([
		{ label: 'Players Online', value: players.length, max: status?.max_clients ?? '?', icon: Users, color: 'text-emerald-400' },
		{ label: 'Current Map', value: status?.map_name ?? '—', icon: Map, color: 'text-blue-400' },
		{ label: 'Game Type', value: gameTypes[status?.game_type] ?? status?.game_type ?? '—', icon: Zap, color: 'text-amber-400' },
		{ label: 'Uptime', value: status?.map_time_start ? timeAgo(status.map_time_start) : '—', icon: Clock, color: 'text-purple-400' },
		{ label: 'Total Warnings', value: summary?.total_warnings ?? '—', icon: AlertTriangle, color: 'text-orange-400' },
		{ label: 'Total Bans', value: (summary?.total_bans ?? 0) + (summary?.total_tempbans ?? 0) || '—', icon: Ban, color: 'text-red-400' },
	]);

	// Sort players into teams for scoreboard
	let redTeam = $derived(players.filter(p => p.team === 'Red').sort((a, b) => (b.score ?? 0) - (a.score ?? 0)));
	let blueTeam = $derived(players.filter(p => p.team === 'Blue').sort((a, b) => (b.score ?? 0) - (a.score ?? 0)));
	let specTeam = $derived(players.filter(p => p.team !== 'Red' && p.team !== 'Blue'));

	async function saveNotes() {
		notesSaving = true;
		notesSaved = false;
		try {
			await api.saveNotes(notes);
			notesSaved = true;
			setTimeout(() => { notesSaved = false; }, 2000);
		} catch (e) { console.error('Failed to save notes:', e); }
		notesSaving = false;
	}

	// Scoreboard expandable player panel
	let expandedPlayer = $state(null); // cid of expanded player
	let actionLoading = $state(false);
	let actionResult = $state(null);
	let kickReason = $state('');
	let banReason = $state('');
	let banDuration = $state('');
	let messageText = $state('');

	function togglePlayer(cid) {
		if (expandedPlayer === cid) {
			expandedPlayer = null;
		} else {
			expandedPlayer = cid;
			kickReason = '';
			banReason = '';
			banDuration = '';
			messageText = '';
		}
	}

	async function doKick(cid, name) {
		actionLoading = true;
		actionResult = null;
		try {
			await api.kickPlayer(cid, kickReason || 'Kicked by admin');
			actionResult = { ok: true, msg: `${stripColors(name)} kicked` };
			kickReason = '';
		} catch (e) { actionResult = { ok: false, msg: e.message }; }
		actionLoading = false;
		setTimeout(() => { actionResult = null; }, 3000);
	}

	async function doBan(cid, name) {
		const dur = banDuration ? parseInt(banDuration) : null;
		actionLoading = true;
		actionResult = null;
		try {
			await api.banPlayer(cid, banReason || 'Banned by admin', dur);
			actionResult = { ok: true, msg: `${stripColors(name)} banned${dur ? ` (${dur}m)` : ''}` };
			banReason = '';
			banDuration = '';
		} catch (e) { actionResult = { ok: false, msg: e.message }; }
		actionLoading = false;
		setTimeout(() => { actionResult = null; }, 3000);
	}

	async function doMessage(cid) {
		if (!messageText.trim()) return;
		actionLoading = true;
		actionResult = null;
		try {
			await api.messagePlayer(cid, messageText);
			actionResult = { ok: true, msg: 'Message sent' };
			messageText = '';
		} catch (e) { actionResult = { ok: false, msg: e.message }; }
		actionLoading = false;
		setTimeout(() => { actionResult = null; }, 3000);
	}
</script>

{#if isMaster()}
<!-- ═══════════════ MASTER DASHBOARD ═══════════════ -->
<div class="space-y-6 animate-fade-in">
	<div>
		<h1 class="text-2xl font-semibold text-surface-100">Master Dashboard</h1>
		<p class="mt-1 text-sm text-surface-500">Overview of all connected game servers</p>
	</div>

	{#if masterLoading}
		<div class="flex items-center justify-center py-20">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
		</div>
	{:else}
		{#if masterError}
			<div class="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">{masterError}</div>
		{/if}

		<!-- Summary cards -->
		<div class="grid grid-cols-1 gap-4 sm:grid-cols-3">
			<div class="card p-5">
				<div class="flex items-center justify-between">
					<div>
						<p class="text-xs font-medium uppercase tracking-wider text-surface-500">Servers Online</p>
						<p class="mt-1 text-2xl font-semibold text-surface-100">{onlineCount} <span class="text-sm text-surface-500">/ {masterServers.length}</span></p>
					</div>
					<div class="rounded-xl bg-surface-800/50 p-3 text-emerald-400">
						<Server class="h-5 w-5" />
					</div>
				</div>
			</div>
			<div class="card p-5">
				<div class="flex items-center justify-between">
					<div>
						<p class="text-xs font-medium uppercase tracking-wider text-surface-500">Total Players</p>
						<p class="mt-1 text-2xl font-semibold text-surface-100">{totalPlayers} <span class="text-sm text-surface-500">/ {totalCapacity}</span></p>
					</div>
					<div class="rounded-xl bg-surface-800/50 p-3 text-blue-400">
						<Users class="h-5 w-5" />
					</div>
				</div>
			</div>
			<div class="card p-5">
				<div class="flex items-center justify-between">
					<div>
						<p class="text-xs font-medium uppercase tracking-wider text-surface-500">Quick-Connect</p>
						<p class="mt-1 text-lg font-semibold text-surface-100"><a href="/pairing" class="text-accent hover:underline">Manage Pairing</a></p>
					</div>
					<div class="rounded-xl bg-surface-800/50 p-3 text-amber-400">
						<Link class="h-5 w-5" />
					</div>
				</div>
			</div>
		</div>

		<!-- Server list -->
		{#if masterServers.length === 0}
			<div class="rounded-xl border border-surface-800 bg-surface-900 p-12 text-center">
				<Server class="mx-auto h-12 w-12 text-surface-600" />
				<h2 class="mt-4 text-lg font-semibold text-surface-300">No servers registered</h2>
				<p class="mt-2 text-sm text-surface-500">Use <a href="/pairing" class="text-accent hover:underline">Pairing</a> to connect game server bots.</p>
			</div>
		{:else}
			<div class="space-y-3">
				<h2 class="text-sm font-medium uppercase tracking-wider text-surface-500">Servers</h2>
				{#each masterServers as server (server.id)}
					<a href="/servers/{server.id}" class="flex items-center gap-4 rounded-xl border border-surface-800 bg-surface-900 p-4 transition-colors hover:border-surface-700 hover:bg-surface-800/50">
						<div class="flex h-9 w-9 items-center justify-center rounded-lg {server.online ? 'bg-emerald-500/10' : 'bg-surface-800'}">
							{#if server.online}
								<Wifi class="h-4 w-4 text-emerald-400" />
							{:else}
								<WifiOff class="h-4 w-4 text-surface-500" />
							{/if}
						</div>
						<div class="flex-1 min-w-0">
							<div class="font-medium text-surface-100 truncate">{server.name}</div>
							<div class="text-xs text-surface-500">{server.address}:{server.port}</div>
						</div>
						{#if server.online}
							<div class="flex items-center gap-4 text-sm text-surface-400">
								{#if server.current_map}
									<span class="flex items-center gap-1.5">
										<Map class="h-3.5 w-3.5" />
										{server.current_map}
									</span>
								{/if}
								<span class="flex items-center gap-1.5">
									<Users class="h-3.5 w-3.5" />
									{server.player_count}/{server.max_clients}
								</span>
							</div>
						{:else}
							<span class="text-xs text-surface-600">Offline</span>
						{/if}
					</a>
				{/each}
			</div>
		{/if}
	{/if}
</div>

{:else}
<!-- ═══════════════ STANDALONE DASHBOARD ═══════════════ -->
<div class="space-y-6 animate-fade-in">
	<div>
		<h1 class="text-2xl font-semibold">{status?.hostname ? stripColors(status.hostname) : 'Dashboard'}</h1>
		<p class="mt-1 text-sm text-surface-500">Server overview and live activity</p>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
		</div>
	{:else}
		<!-- Stat Cards -->
		<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-6">
			{#each statCards as card}
				{#if card.label === 'Current Map' && isAdmin}
					<button class="card p-5 text-left cursor-pointer transition-all hover:ring-2 hover:ring-accent/50 hover:bg-surface-800/30" onclick={openMapModal}>
						<div class="flex items-center justify-between">
							<div>
								<p class="text-xs font-medium uppercase tracking-wider text-surface-500">{card.label}</p>
								<p class="mt-1 text-2xl font-semibold text-surface-100">
									{card.value}
								</p>
							</div>
							<div class="rounded-xl bg-surface-800/50 p-3 {card.color}">
								<card.icon class="h-5 w-5" />
							</div>
						</div>
						<p class="mt-2 text-[10px] uppercase tracking-wider text-accent/60">Click to change map</p>
					</button>
				{:else}
					<div class="card p-5">
						<div class="flex items-center justify-between">
							<div>
								<p class="text-xs font-medium uppercase tracking-wider text-surface-500">{card.label}</p>
								<p class="mt-1 text-2xl font-semibold text-surface-100">
									{card.value}{#if card.max}<span class="text-sm text-surface-500">/{card.max}</span>{/if}
								</p>
							</div>
							<div class="rounded-xl bg-surface-800/50 p-3 {card.color}">
								<card.icon class="h-5 w-5" />
							</div>
						</div>
					</div>
				{/if}
			{/each}
		</div>

		<!-- Quick Access Panel -->
		<div class="card p-4">
			<h2 class="text-xs font-medium uppercase tracking-wider text-surface-500 mb-3">Quick Access</h2>
			<div class="flex flex-wrap gap-2">
				<a href="/players" class="inline-flex items-center gap-2 rounded-lg bg-surface-800/50 px-4 py-2 text-sm text-surface-300 hover:bg-surface-700/50 hover:text-surface-100 transition-colors">
					<Users class="h-4 w-4" /> Players
				</a>
				<a href="/penalties" class="inline-flex items-center gap-2 rounded-lg bg-surface-800/50 px-4 py-2 text-sm text-surface-300 hover:bg-surface-700/50 hover:text-surface-100 transition-colors">
					<Shield class="h-4 w-4" /> Penalties
				</a>
				<a href="/stats" class="inline-flex items-center gap-2 rounded-lg bg-surface-800/50 px-4 py-2 text-sm text-surface-300 hover:bg-surface-700/50 hover:text-surface-100 transition-colors">
					<Zap class="h-4 w-4" /> Stats
				</a>
				<a href="/console" class="inline-flex items-center gap-2 rounded-lg bg-surface-800/50 px-4 py-2 text-sm text-surface-300 hover:bg-surface-700/50 hover:text-surface-100 transition-colors">
					<ExternalLink class="h-4 w-4" /> Console
				</a>
				<a href="/audit-log" class="inline-flex items-center gap-2 rounded-lg bg-surface-800/50 px-4 py-2 text-sm text-surface-300 hover:bg-surface-700/50 hover:text-surface-100 transition-colors">
					<AlertTriangle class="h-4 w-4" /> Audit Log
				</a>
			</div>
		</div>

		<div class="grid grid-cols-1 gap-6 xl:grid-cols-2">
			<!-- Live Scoreboard -->
			<div class="card">
				<div class="border-b border-surface-800 px-5 py-4">
					<h2 class="text-sm font-medium text-surface-300">Live Scoreboard</h2>
				</div>
				{#if players.length === 0}
					<div class="px-5 py-10 text-center text-sm text-surface-500">No players online</div>
				{:else}
					<div class="p-4 space-y-4">
						{#snippet playerRow(p, teamColor)}
							<div class="rounded-lg transition-colors {expandedPlayer === p.cid ? 'bg-surface-800/40' : ''}">
								<button class="w-full flex items-center gap-2 px-3 py-1.5 text-sm hover:bg-surface-800/40 rounded-lg transition-colors text-left"
									onclick={() => isAdmin && togglePlayer(p.cid)}>
									<span class="flex-1 min-w-0 flex flex-col">
										<span class="flex items-center gap-2">
											<span class="text-surface-200 font-medium truncate">{stripColors(p.current_name || p.name)}</span>
											{#if p.auth}
												<span class="text-[10px] uppercase font-bold px-1.5 py-0.5 rounded bg-purple-500/15 text-purple-400">{p.auth}</span>
											{/if}
											{#if p.group_name && p.group_name !== 'Guest'}
												<span class="text-[10px] uppercase font-bold px-1.5 py-0.5 rounded bg-accent/15 text-accent">{p.group_name}</span>
											{/if}
										</span>
										{#if p.current_name && stripColors(p.current_name) !== stripColors(p.name)}
											<span class="text-[10px] text-surface-500 truncate">aka {stripColors(p.name)}</span>
										{/if}
									</span>
									<span class="text-surface-400 tabular-nums w-12 text-right">{p.score ?? 0}</span>
									<span class="text-surface-500 tabular-nums w-16 text-right text-xs">{p.ping ?? '—'}ms</span>
									{#if isAdmin}
										<span class="text-surface-500">
											{#if expandedPlayer === p.cid}
												<ChevronUp class="h-4 w-4" />
											{:else}
												<ChevronDown class="h-4 w-4" />
											{/if}
										</span>
									{/if}
								</button>
								{#if isAdmin && expandedPlayer === p.cid}
									<div class="px-3 pb-3 pt-1 space-y-2.5 animate-fade-in">
										<div class="text-xs text-surface-500">
											Slot: {p.cid} &nbsp;|&nbsp; ID: {p.id} &nbsp;|&nbsp; Connected: {p.connected ? new Date(p.connected).toLocaleDateString() : '—'}
										</div>
										<div class="flex items-center gap-2">
											<a href="/players/{p.id}" class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-accent/15 text-accent hover:bg-accent/25 transition-colors">
												<Eye class="h-3 w-3" /> View Profile
											</a>
											<button onclick={() => doKick(p.cid, p.name)} disabled={actionLoading}
												class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-amber-500/15 text-amber-400 hover:bg-amber-500/25 transition-colors disabled:opacity-40">
												<UserX class="h-3 w-3" /> Kick
											</button>
											<button onclick={() => doBan(p.cid, p.name)} disabled={actionLoading}
												class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-red-500/15 text-red-400 hover:bg-red-500/25 transition-colors disabled:opacity-40">
												<ShieldBan class="h-3 w-3" /> Ban
											</button>
										</div>
										<!-- Kick reason -->
										<div class="flex items-center gap-2">
											<input type="text" bind:value={kickReason} placeholder="Kick reason (optional)"
												onkeydown={(e) => e.key === 'Enter' && doKick(p.cid, p.name)}
												class="flex-1 px-2.5 py-1.5 bg-surface-900 border border-surface-700 rounded-md text-xs text-surface-200 placeholder-surface-600 focus:border-amber-500 focus:outline-none" />
											<button onclick={() => doKick(p.cid, p.name)} disabled={actionLoading}
												class="px-3 py-1.5 rounded-md text-xs font-medium bg-amber-500/20 text-amber-400 hover:bg-amber-500/30 transition-colors disabled:opacity-40">
												Kick
											</button>
										</div>
										<!-- Ban reason + duration -->
										<div class="flex items-center gap-2">
											<input type="text" bind:value={banReason} placeholder="Ban reason (optional)"
												onkeydown={(e) => e.key === 'Enter' && doBan(p.cid, p.name)}
												class="flex-1 px-2.5 py-1.5 bg-surface-900 border border-surface-700 rounded-md text-xs text-surface-200 placeholder-surface-600 focus:border-red-500 focus:outline-none" />
											<input type="number" bind:value={banDuration} placeholder="Mins (perm if empty)" min="1"
												class="w-28 px-2.5 py-1.5 bg-surface-900 border border-surface-700 rounded-md text-xs text-surface-200 placeholder-surface-600 focus:border-red-500 focus:outline-none" />
											<button onclick={() => doBan(p.cid, p.name)} disabled={actionLoading}
												class="px-3 py-1.5 rounded-md text-xs font-medium bg-red-500/20 text-red-400 hover:bg-red-500/30 transition-colors disabled:opacity-40">
												Ban
											</button>
										</div>
										<!-- Private message -->
										<div class="flex items-center gap-2">
											<input type="text" bind:value={messageText} placeholder="Send private message..."
												onkeydown={(e) => e.key === 'Enter' && doMessage(p.cid)}
												class="flex-1 px-2.5 py-1.5 bg-surface-900 border border-surface-700 rounded-md text-xs text-surface-200 placeholder-surface-600 focus:border-accent focus:outline-none" />
											<button onclick={() => doMessage(p.cid)} disabled={actionLoading || !messageText.trim()}
												class="p-1.5 rounded-md bg-accent/20 text-accent hover:bg-accent/30 transition-colors disabled:opacity-40">
												<Send class="h-3.5 w-3.5" />
											</button>
										</div>
									</div>
								{/if}
							</div>
						{/snippet}

						{#if redTeam.length > 0}
							<div>
								<div class="flex items-center justify-between mb-2">
									<div class="text-xs font-semibold uppercase text-red-400">Red Team ({redTeam.length})</div>
									<div class="flex gap-4 text-[10px] uppercase tracking-wider text-surface-600 pr-8">
										<span class="w-12 text-right">Score</span>
										<span class="w-16 text-right">Ping</span>
									</div>
								</div>
								<div class="space-y-0.5">
									{#each redTeam as p}
										{@render playerRow(p, 'red')}
									{/each}
								</div>
							</div>
						{/if}
						{#if blueTeam.length > 0}
							<div>
								<div class="flex items-center justify-between mb-2">
									<div class="text-xs font-semibold uppercase text-blue-400">Blue Team ({blueTeam.length})</div>
									<div class="flex gap-4 text-[10px] uppercase tracking-wider text-surface-600 pr-8">
										<span class="w-12 text-right">Score</span>
										<span class="w-16 text-right">Ping</span>
									</div>
								</div>
								<div class="space-y-0.5">
									{#each blueTeam as p}
										{@render playerRow(p, 'blue')}
									{/each}
								</div>
							</div>
						{/if}
						{#if specTeam.length > 0}
							<div>
								<div class="flex items-center justify-between mb-2">
									<div class="text-xs font-semibold uppercase text-surface-500">Spectators ({specTeam.length})</div>
									<div class="flex gap-4 text-[10px] uppercase tracking-wider text-surface-600 pr-8">
										<span class="w-12 text-right">Score</span>
										<span class="w-16 text-right">Ping</span>
									</div>
								</div>
								<div class="space-y-0.5">
									{#each specTeam as p}
										{@render playerRow(p, 'spec')}
									{/each}
								</div>
							</div>
						{/if}
					</div>
					{#if players.length > 0}
						<div class="border-t border-surface-800 px-5 py-3 text-center">
							<a href="/players" class="text-xs font-medium text-accent hover:text-accent-400">View all players →</a>
						</div>
					{/if}
				{/if}
			</div>

			<!-- Live Chat -->
			<div class="card">
				<div class="border-b border-surface-800 px-5 py-4">
					<div class="flex items-center gap-2">
						<MessageSquare class="h-4 w-4 text-emerald-400" />
						<h2 class="text-sm font-medium text-surface-300">Live Chat</h2>
					</div>
				</div>
				{#if recentChat.length === 0}
					<div class="px-5 py-10 text-center text-sm text-surface-500">No chat messages yet…</div>
				{:else}
					<div class="max-h-80 overflow-y-auto divide-y divide-surface-800/50">
						{#each recentChat.slice(0, 30) as msg}
							<div class="px-5 py-2">
								<div class="flex items-center justify-between">
									<div class="flex items-center gap-2">
										<span class="text-sm font-medium text-surface-200">{stripColors(msg.client_name)}</span>
										{#if msg.channel === 'team'}
											<span class="text-[10px] uppercase font-bold text-amber-500">[team]</span>
										{/if}
									</div>
									<span class="text-xs text-surface-600">{timeAgo(msg.time_add)}</span>
								</div>
								<p class="mt-0.5 text-sm text-surface-400">{msg.message}</p>
							</div>
						{/each}
					</div>
				{/if}
			</div>

			<!-- Map Vote History -->
			<div class="card">
				<div class="border-b border-surface-800 px-5 py-4">
					<div class="flex items-center gap-2">
						<Vote class="h-4 w-4 text-amber-400" />
						<h2 class="text-sm font-medium text-surface-300">Vote History</h2>
					</div>
				</div>
				{#if recentVotes.length === 0}
					<div class="px-5 py-10 text-center text-sm text-surface-500">No votes recorded yet…</div>
				{:else}
					<div class="max-h-72 overflow-y-auto divide-y divide-surface-800/50">
						{#each recentVotes as vote}
							<div class="px-5 py-2.5">
								<div class="flex items-center justify-between">
									<div class="flex items-center gap-2">
										<span class="badge-blue">{vote.vote_type}</span>
										<span class="text-sm text-surface-300">{stripColors(vote.client_name)}</span>
									</div>
									<span class="text-xs text-surface-600">{timeAgo(vote.time_add)}</span>
								</div>
								{#if vote.vote_data}
									<p class="mt-0.5 text-xs text-surface-500">{vote.vote_data}</p>
								{/if}
							</div>
						{/each}
					</div>
				{/if}
			</div>

			<!-- Personal Notes -->
			<div class="card">
				<div class="border-b border-surface-800 px-5 py-4">
					<div class="flex items-center justify-between">
						<div class="flex items-center gap-2">
							<StickyNote class="h-4 w-4 text-yellow-400" />
							<h2 class="text-sm font-medium text-surface-300">Personal Notes</h2>
						</div>
						<button
							class="rounded-lg bg-accent/10 px-3 py-1 text-xs font-medium text-accent hover:bg-accent/20 transition-colors disabled:opacity-50"
							onclick={saveNotes}
							disabled={notesSaving}
						>
							{notesSaved ? 'Saved!' : notesSaving ? 'Saving…' : 'Save'}
						</button>
					</div>
				</div>
				<div class="p-4">
					<textarea
						class="w-full h-40 rounded-lg bg-surface-800/50 border border-surface-700 px-3 py-2 text-sm text-surface-200 placeholder-surface-600 focus:border-accent focus:outline-none resize-none"
						placeholder="Write notes about players, events, or anything…"
						bind:value={notes}
					></textarea>
				</div>
			</div>
		</div>

		<!-- Recent Activity -->
		<div class="card">
			<div class="border-b border-surface-800 px-5 py-4">
				<div class="flex items-center gap-2">
					<Activity class="h-4 w-4 text-emerald-400 animate-pulse-soft" />
					<h2 class="text-sm font-medium text-surface-300">Recent Activity</h2>
				</div>
			</div>
			{#if recentEvents.length === 0}
				<div class="px-5 py-10 text-center text-sm text-surface-500">Waiting for events…</div>
			{:else}
				<div class="max-h-80 overflow-y-auto divide-y divide-surface-800/50">
					{#each recentEvents as evt}
						<div class="px-5 py-2.5">
							<div class="flex items-center justify-between">
								<span class="badge-blue">{evt.type ?? 'event'}</span>
								<span class="text-xs text-surface-600">{timeAgo(evt.time)}</span>
							</div>
							<p class="mt-1 truncate text-xs text-surface-400">
								{#if evt.client_name}
									<span class="text-surface-300">{stripColors(evt.client_name)}</span>
								{/if}
								{#if evt.type === 'EVT_CLIENT_KILL' && evt.target_name}
									killed <span class="text-surface-300">{stripColors(evt.target_name)}</span>
									{#if evt.data?.weapon} with {evt.data.weapon}{/if}
									{#if evt.data?.hit_location} ({evt.data.hit_location}){/if}
								{:else if evt.type === 'EVT_CLIENT_DISCONNECT'}
									disconnected
								{:else if evt.type === 'EVT_CLIENT_AUTH' || evt.type === 'EVT_CLIENT_CONNECT'}
									connected
								{:else if evt.type === 'EVT_GAME_MAP_CHANGE' && evt.data?.new_map}
									Map changed to {evt.data.new_map}
								{:else if evt.data?.text}
									{stripColors(evt.data.text)}
								{:else if evt.data && typeof evt.data === 'object'}
									<span class="font-mono">{JSON.stringify(evt.data).slice(0, 100)}</span>
								{/if}
							</p>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	{/if}
</div>

<!-- Map Selection Modal -->
{#if showMapModal}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
		 onclick={() => showMapModal = false}
		 onkeydown={(e) => e.key === 'Escape' && (showMapModal = false)}
		 role="dialog"
		 tabindex="-1">
		<div class="card w-full max-w-lg p-0 animate-slide-up max-h-[80vh] flex flex-col" onclick={(e) => e.stopPropagation()}>
			<!-- Header -->
			<div class="flex items-center justify-between border-b border-surface-800 px-5 py-4">
				<div>
					<h2 class="text-base font-semibold text-surface-100">Change Map</h2>
					<p class="text-xs text-surface-500 mt-0.5">Current: <span class="text-accent">{status?.map_name ?? '—'}</span></p>
				</div>
				<button class="rounded-lg p-1.5 text-surface-500 hover:bg-surface-800 hover:text-surface-300 transition-colors" onclick={() => showMapModal = false}>
					<X class="h-4 w-4" />
				</button>
			</div>

			<!-- Search -->
			<div class="px-5 py-3 border-b border-surface-800/50">
				<div class="relative">
					<Search class="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-surface-500" />
					<input
						type="text"
						class="input pl-9"
						placeholder="Search maps…"
						bind:value={mapSearch}
					/>
				</div>
			</div>

			<!-- Map List -->
			<div class="flex-1 overflow-y-auto min-h-0" style="max-height: 45vh;">
				{#if mapsLoading}
					<div class="flex items-center justify-center py-12">
						<div class="h-6 w-6 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
					</div>
				{:else if filteredMaps.length === 0}
					<div class="px-5 py-10 text-center text-sm text-surface-500">
						{mapList.length === 0 ? 'No maps found on server' : 'No maps match your search'}
					</div>
				{:else}
					<div class="divide-y divide-surface-800/30">
						{#each filteredMaps as map}
							<button
								class="w-full px-5 py-2.5 text-left text-sm transition-colors flex items-center justify-between
									{map === selectedMap ? 'bg-accent/10 text-accent' : map === status?.map_name ? 'bg-surface-800/30 text-blue-400' : 'text-surface-300 hover:bg-surface-800/30 hover:text-surface-100'}"
								onclick={() => selectedMap = (selectedMap === map ? null : map)}
							>
								<span class="font-medium">{map}</span>
								{#if map === status?.map_name}
									<span class="text-[10px] uppercase font-bold text-blue-400/70">current</span>
								{:else if map === selectedMap}
									<span class="text-[10px] uppercase font-bold text-accent">selected</span>
								{/if}
							</button>
						{/each}
					</div>
				{/if}
			</div>

			<!-- Actions -->
			{#if selectedMap}
				<div class="border-t border-surface-800 px-5 py-4 space-y-3">
					<p class="text-xs text-surface-500">Selected: <span class="text-surface-200 font-medium">{selectedMap}</span></p>
					<div class="flex gap-3">
						<button
							class="btn-primary flex-1 flex items-center justify-center gap-2 text-sm"
							onclick={() => doMapAction('change')}
							disabled={mapActionLoading}
						>
							<ArrowRight class="h-4 w-4" />
							Change Map Now
						</button>
						<button
							class="btn-secondary flex-1 flex items-center justify-center gap-2 text-sm"
							onclick={() => doMapAction('setnext')}
							disabled={mapActionLoading}
						>
							<SkipForward class="h-4 w-4" />
							Set as Next Map
						</button>
					</div>
				</div>
			{/if}

			<!-- Result feedback -->
			{#if mapActionResult}
				<div class="border-t border-surface-800 px-5 py-3">
					<p class="text-sm {mapActionResult.success ? 'text-emerald-400' : 'text-red-400'}">
						{mapActionResult.message}
					</p>
				</div>
			{/if}
		</div>
	</div>
{/if}

<!-- Action result toast -->
{#if actionResult}
	<div class="fixed bottom-6 right-6 z-50 px-4 py-2.5 rounded-lg shadow-lg text-sm font-medium animate-fade-in
		{actionResult.ok ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30' : 'bg-red-500/20 text-red-400 border border-red-500/30'}">
		{actionResult.msg}
	</div>
{/if}

{/if}
<!-- end master/standalone switch -->
