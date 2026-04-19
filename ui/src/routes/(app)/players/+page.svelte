<script>
	import { api } from '$lib/api.svelte.js';
	import { getOnlinePlayers, isInitialized } from '$lib/live.svelte.js';
	import { stripColors, timeAgo } from '$lib/utils.js';
	import { Search, Ban, MessageSquare, ExternalLink, RefreshCw } from 'lucide-svelte';

	let players = $derived(getOnlinePlayers());
	let searchQuery = $state('');
	let searchResults = $state([]);
	let loading = $derived(!isInitialized());
	let searching = $state(false);

	async function search() {
		if (searchQuery.length < 2) { searchResults = []; return; }
		searching = true;
		try {
			searchResults = await api.searchClients(searchQuery);
		} catch (e) {
			console.error(e);
		}
		searching = false;
	}

	let displayList = $derived(searchResults.length > 0 ? searchResults : players);
	let isSearch = $derived(searchResults.length > 0);
</script>

<div class="space-y-6 animate-fade-in">
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-2xl font-semibold">Players</h1>
			<p class="mt-1 text-sm text-surface-500">
				<span class="inline-flex items-center gap-1.5">
					<span class="h-1.5 w-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
					{players.length} currently online
				</span>
				<span class="text-surface-600 ml-1">· auto-updating</span>
			</p>
		</div>
	</div>

	<!-- Search -->
	<div class="card p-4">
		<div class="flex gap-3">
			<div class="relative flex-1">
				<Search class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-surface-500" />
				<input
					type="text"
					bind:value={searchQuery}
					oninput={search}
					class="input pl-10"
					placeholder="Search players by name (database lookup)…"
				/>
			</div>
		</div>
		{#if isSearch}
			<p class="mt-2 text-xs text-surface-500">Showing {searchResults.length} database results for "{searchQuery}"</p>
		{/if}
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
		</div>
	{:else}
		<!-- Player Table -->
		<div class="card overflow-hidden">
			<table class="w-full text-sm">
				<thead>
					<tr class="border-b border-surface-800 text-left text-xs font-medium uppercase tracking-wider text-surface-500">
						<th class="px-5 py-3">Slot</th>
						<th class="px-5 py-3">Name</th>
						<th class="px-5 py-3">IP</th>
						<th class="px-5 py-3">Group</th>
						<th class="px-5 py-3">Connected</th>
						<th class="px-5 py-3 text-right">Actions</th>
					</tr>
				</thead>
				<tbody class="divide-y divide-surface-800/50">
					{#each displayList as p}
						<tr class="hover:bg-surface-800/30 transition-colors">
							<td class="px-5 py-3">
								<span class="font-mono text-surface-500">{p.cid ?? '—'}</span>
							</td>
							<td class="px-5 py-3">
								<a href="/players/{p.id}" class="font-medium text-surface-200 hover:text-accent transition-colors">
									{stripColors(p.name)}
								</a>
							</td>
							<td class="px-5 py-3 font-mono text-xs text-surface-500">{p.ip ?? '—'}</td>
							<td class="px-5 py-3">
								<span class="badge-gray">{p.group_name ?? p.group_bits ?? '—'}</span>
							</td>
							<td class="px-5 py-3 text-xs text-surface-500">{p.time_add ? timeAgo(p.time_add) : '—'}</td>
							<td class="px-5 py-3 text-right">
								<div class="flex items-center justify-end gap-1">
									<a href="/players/{p.id}" class="btn-ghost btn-sm" title="View detail">
										<ExternalLink class="h-3.5 w-3.5" />
									</a>
								</div>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
			{#if displayList.length === 0}
				<div class="px-5 py-10 text-center text-sm text-surface-500">
					{isSearch ? 'No players found' : 'No players online'}
				</div>
			{/if}
		</div>
	{/if}
</div>
