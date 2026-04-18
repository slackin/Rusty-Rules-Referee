<script>
	import { onMount } from 'svelte';
	import { api } from '$lib/api.svelte.js';
	import { Trophy, Target, MapPin, Crosshair } from 'lucide-svelte';

	let tab = $state('leaderboard');
	let leaderboard = $state([]);
	let weapons = $state([]);
	let maps = $state([]);
	let loading = $state(true);

	onMount(async () => {
		try {
			leaderboard = await api.leaderboard(50);
		} catch (e) {
			console.error(e);
		}
		loading = false;
	});

	async function switchTab(t) {
		tab = t;
		loading = true;
		try {
			if (t === 'leaderboard' && leaderboard.length === 0) leaderboard = await api.leaderboard(50);
			if (t === 'weapons' && weapons.length === 0) weapons = await api.weaponStats();
			if (t === 'maps' && maps.length === 0) maps = await api.mapStats();
		} catch (e) {
			console.error(e);
		}
		loading = false;
	}
</script>

<div class="space-y-6 animate-fade-in">
	<div>
		<h1 class="text-2xl font-semibold">Statistics</h1>
		<p class="mt-1 text-sm text-surface-500">XLR player statistics and leaderboards</p>
	</div>

	<!-- Tabs -->
	<div class="flex gap-1 rounded-lg bg-surface-900 p-1 border border-surface-800 w-fit">
		{#each [['leaderboard', 'Leaderboard', Trophy], ['weapons', 'Weapons', Crosshair], ['maps', 'Maps', MapPin]] as [key, label, Icon]}
			<button
				class="flex items-center gap-2 rounded-md px-4 py-2 text-sm transition-colors {tab === key ? 'bg-surface-800 text-surface-100 font-medium' : 'text-surface-500 hover:text-surface-300'}"
				onclick={() => switchTab(key)}
			>
				<Icon class="h-4 w-4" />
				{label}
			</button>
		{/each}
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
		</div>
	{:else if tab === 'leaderboard'}
		<div class="card overflow-hidden">
			<table class="w-full text-sm">
				<thead>
					<tr class="border-b border-surface-800 text-left text-xs font-medium uppercase tracking-wider text-surface-500">
						<th class="px-5 py-3 w-12">#</th>
						<th class="px-5 py-3">Player</th>
						<th class="px-5 py-3">Kills</th>
						<th class="px-5 py-3">Deaths</th>
						<th class="px-5 py-3">K/D</th>
						<th class="px-5 py-3">Skill</th>
						<th class="px-5 py-3">Rounds</th>
					</tr>
				</thead>
				<tbody class="divide-y divide-surface-800/50">
					{#each leaderboard as row, i}
						<tr class="hover:bg-surface-800/30 transition-colors">
							<td class="px-5 py-3 font-mono text-surface-500">{i + 1}</td>
							<td class="px-5 py-3">
								<a href="/players/{row.client_id}" class="font-medium text-surface-200 hover:text-accent">{row.name ?? `#${row.client_id}`}</a>
							</td>
							<td class="px-5 py-3 text-emerald-400">{row.kills?.toLocaleString() ?? 0}</td>
							<td class="px-5 py-3 text-red-400">{row.deaths?.toLocaleString() ?? 0}</td>
							<td class="px-5 py-3 font-medium text-surface-200">{row.deaths > 0 ? (row.kills / row.deaths).toFixed(2) : '—'}</td>
							<td class="px-5 py-3">
								<span class="font-semibold text-amber-400">{row.skill?.toFixed(1) ?? '—'}</span>
							</td>
							<td class="px-5 py-3 text-surface-500">{row.rounds?.toLocaleString() ?? 0}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{:else if tab === 'weapons'}
		<div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
			{#each weapons as w}
				<div class="card p-4">
					<div class="flex items-center justify-between">
						<h3 class="font-medium text-surface-200">{w.name ?? w.weapon_id}</h3>
						<span class="badge-blue">{w.kills?.toLocaleString() ?? 0} kills</span>
					</div>
				</div>
			{/each}
			{#if weapons.length === 0}
				<div class="col-span-full text-center py-10 text-sm text-surface-500">No weapon stats available</div>
			{/if}
		</div>
	{:else if tab === 'maps'}
		<div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
			{#each maps as m}
				<div class="card p-4">
					<div class="flex items-center justify-between">
						<h3 class="font-medium text-surface-200">{m.name ?? m.map_id}</h3>
						<span class="badge-green">{m.rounds?.toLocaleString() ?? 0} rounds</span>
					</div>
				</div>
			{/each}
			{#if maps.length === 0}
				<div class="col-span-full text-center py-10 text-sm text-surface-500">No map stats available</div>
			{/if}
		</div>
	{/if}
</div>
