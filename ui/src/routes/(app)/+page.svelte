<script>
	import { onMount } from 'svelte';
	import { api } from '$lib/api.svelte.js';
	import { onEvent } from '$lib/ws.js';
	import { stripColors, timeAgo } from '$lib/utils.js';
	import { Users, Map, Clock, Activity, Zap } from 'lucide-svelte';

	let status = $state(null);
	let players = $state([]);
	let recentEvents = $state([]);
	let loading = $state(true);

	onMount(async () => {
		try {
			const [s, p] = await Promise.all([api.serverStatus(), api.players()]);
			status = s;
			players = p;
		} catch (e) {
			console.error('Failed to load dashboard:', e);
		}
		loading = false;

		return onEvent((evt) => {
			recentEvents = [evt, ...recentEvents.slice(0, 49)];
		});
	});

	let statCards = $derived([
		{ label: 'Players Online', value: players.length, max: status?.max_clients ?? '?', icon: Users, color: 'text-emerald-400' },
		{ label: 'Current Map', value: status?.map_name ?? '—', icon: Map, color: 'text-blue-400' },
		{ label: 'Game Type', value: status?.game_type ?? '—', icon: Zap, color: 'text-amber-400' },
		{ label: 'Uptime', value: status?.map_time_start ? timeAgo(status.map_time_start) : '—', icon: Clock, color: 'text-purple-400' }
	]);
</script>

<div class="space-y-6 animate-fade-in">
	<div>
		<h1 class="text-2xl font-semibold">Dashboard</h1>
		<p class="mt-1 text-sm text-surface-500">Server overview and live activity</p>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
		</div>
	{:else}
		<!-- Stat Cards -->
		<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
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

		<div class="grid grid-cols-1 gap-6 xl:grid-cols-2">
			<!-- Online Players -->
			<div class="card">
				<div class="border-b border-surface-800 px-5 py-4">
					<h2 class="text-sm font-medium text-surface-300">Online Players</h2>
				</div>
				{#if players.length === 0}
					<div class="px-5 py-10 text-center text-sm text-surface-500">No players online</div>
				{:else}
					<div class="divide-y divide-surface-800/50">
						{#each players.slice(0, 12) as p}
							<a href="/players/{p.id}" class="flex items-center gap-3 px-5 py-3 hover:bg-surface-800/30 transition-colors">
								<div class="flex h-8 w-8 items-center justify-center rounded-full bg-surface-800 text-xs font-medium text-surface-400">
									{p.cid}
								</div>
								<div class="flex-1 min-w-0">
									<div class="truncate text-sm font-medium text-surface-200">{stripColors(p.name)}</div>
									<div class="text-xs text-surface-500">{p.ip ?? 'unknown'}</div>
								</div>
								<span class="badge-green">online</span>
							</a>
						{/each}
					</div>
					{#if players.length > 12}
						<div class="border-t border-surface-800 px-5 py-3 text-center">
							<a href="/players" class="text-xs font-medium text-accent hover:text-accent-400">View all {players.length} players →</a>
						</div>
					{/if}
				{/if}
			</div>

			<!-- Live Events -->
			<div class="card">
				<div class="border-b border-surface-800 px-5 py-4">
					<div class="flex items-center gap-2">
						<Activity class="h-4 w-4 text-emerald-400 animate-pulse-soft" />
						<h2 class="text-sm font-medium text-surface-300">Live Events</h2>
					</div>
				</div>
				{#if recentEvents.length === 0}
					<div class="px-5 py-10 text-center text-sm text-surface-500">Waiting for events…</div>
				{:else}
					<div class="max-h-96 overflow-y-auto divide-y divide-surface-800/50">
						{#each recentEvents as evt}
							<div class="px-5 py-2.5">
								<div class="flex items-center justify-between">
									<span class="badge-blue">{evt.event_type ?? evt.type ?? 'event'}</span>
									<span class="text-xs text-surface-600">{timeAgo(evt.timestamp)}</span>
								</div>
								{#if evt.data}
									<p class="mt-1 truncate text-xs text-surface-500 font-mono">{JSON.stringify(evt.data).slice(0, 120)}</p>
								{/if}
							</div>
						{/each}
					</div>
				{/if}
			</div>
		</div>
	{/if}
</div>
