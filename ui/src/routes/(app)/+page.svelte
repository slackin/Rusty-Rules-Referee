<script>
	import { api } from '$lib/api.svelte.js';
	import { getServerStatus, getOnlinePlayers, getRecentEvents, getRecentChat, getRecentVotes, isInitialized } from '$lib/live.svelte.js';
	import { stripColors, timeAgo } from '$lib/utils.js';
	import { Users, Map, Clock, Activity, Zap, Shield, Ban, AlertTriangle, MessageSquare, Vote, StickyNote, ExternalLink } from 'lucide-svelte';

	let status = $derived(getServerStatus());
	let players = $derived(getOnlinePlayers());
	let recentEvents = $derived(getRecentEvents());
	let recentChat = $derived(getRecentChat());
	let recentVotes = $derived(getRecentVotes());
	let loading = $derived(!isInitialized());

	let summary = $state(null);
	let notes = $state('');
	let notesSaving = $state(false);
	let notesSaved = $state(false);

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
</script>

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
						{#if redTeam.length > 0}
							<div>
								<div class="text-xs font-semibold uppercase text-red-400 mb-2">Red Team ({redTeam.length})</div>
								<table class="w-full text-sm">
									<thead>
										<tr class="text-xs text-surface-500 border-b border-surface-800/50">
											<th class="text-left py-1 px-2">Player</th>
											<th class="text-right py-1 px-2">Score</th>
											<th class="text-right py-1 px-2">Ping</th>
										</tr>
									</thead>
									<tbody>
										{#each redTeam as p}
											<tr class="hover:bg-surface-800/30 transition-colors">
												<td class="py-1.5 px-2">
													<a href="/players/{p.id}" class="text-surface-200 hover:text-accent">{stripColors(p.name)}</a>
												</td>
												<td class="text-right py-1.5 px-2 text-surface-400">{p.score ?? 0}</td>
												<td class="text-right py-1.5 px-2 text-surface-500">{p.ping ?? '—'}ms</td>
											</tr>
										{/each}
									</tbody>
								</table>
							</div>
						{/if}
						{#if blueTeam.length > 0}
							<div>
								<div class="text-xs font-semibold uppercase text-blue-400 mb-2">Blue Team ({blueTeam.length})</div>
								<table class="w-full text-sm">
									<thead>
										<tr class="text-xs text-surface-500 border-b border-surface-800/50">
											<th class="text-left py-1 px-2">Player</th>
											<th class="text-right py-1 px-2">Score</th>
											<th class="text-right py-1 px-2">Ping</th>
										</tr>
									</thead>
									<tbody>
										{#each blueTeam as p}
											<tr class="hover:bg-surface-800/30 transition-colors">
												<td class="py-1.5 px-2">
													<a href="/players/{p.id}" class="text-surface-200 hover:text-accent">{stripColors(p.name)}</a>
												</td>
												<td class="text-right py-1.5 px-2 text-surface-400">{p.score ?? 0}</td>
												<td class="text-right py-1.5 px-2 text-surface-500">{p.ping ?? '—'}ms</td>
											</tr>
										{/each}
									</tbody>
								</table>
							</div>
						{/if}
						{#if specTeam.length > 0}
							<div>
								<div class="text-xs font-semibold uppercase text-surface-500 mb-2">Spectators ({specTeam.length})</div>
								<div class="flex flex-wrap gap-2">
									{#each specTeam as p}
										<a href="/players/{p.id}" class="text-xs text-surface-400 bg-surface-800/40 rounded px-2 py-1 hover:text-accent">{stripColors(p.name)}</a>
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
