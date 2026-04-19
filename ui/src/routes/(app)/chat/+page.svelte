<script>
	import { onMount } from 'svelte';
	import { api } from '$lib/api.svelte.js';
	import { stripColors, timeAgo } from '$lib/utils.js';
	import { Search, ChevronDown } from 'lucide-svelte';

	let messages = $state([]);
	let loading = $state(true);
	let loadingMore = $state(false);
	let hasMore = $state(true);

	let searchQuery = $state('');
	let playerFilter = $state('');
	let playerResults = $state([]);
	let selectedPlayer = $state(null);
	let showPlayerDropdown = $state(false);

	const PAGE_SIZE = 100;

	onMount(async () => {
		await loadMessages();
	});

	async function loadMessages(append = false) {
		if (append) loadingMore = true; else loading = true;
		try {
			const beforeId = append && messages.length > 0 ? messages[messages.length - 1].id : null;
			const result = await api.searchChat({
				limit: PAGE_SIZE,
				beforeId,
				query: searchQuery,
				clientId: selectedPlayer?.id ?? null
			});
			if (append) {
				messages = [...messages, ...result];
			} else {
				messages = result;
			}
			hasMore = result.length === PAGE_SIZE;
		} catch (e) {
			console.error(e);
		}
		loading = false;
		loadingMore = false;
	}

	async function doSearch() {
		hasMore = true;
		await loadMessages();
	}

	async function searchPlayers() {
		if (playerFilter.length < 2) { playerResults = []; return; }
		try {
			playerResults = await api.searchClients(playerFilter);
			showPlayerDropdown = true;
		} catch (e) {
			console.error(e);
		}
	}

	function selectPlayer(p) {
		selectedPlayer = p;
		playerFilter = stripColors(p.name);
		showPlayerDropdown = false;
		playerResults = [];
		doSearch();
	}

	function clearPlayer() {
		selectedPlayer = null;
		playerFilter = '';
		playerResults = [];
		showPlayerDropdown = false;
		doSearch();
	}

	function channelBadge(ch) {
		switch (ch) {
			case 'SAY': return 'badge-blue';
			case 'TEAM': return 'badge-green';
			case 'PM': return 'badge-yellow';
			default: return 'badge-gray';
		}
	}
</script>

<div class="space-y-6 animate-fade-in">
	<div>
		<h1 class="text-2xl font-semibold">Chat Logs</h1>
		<p class="mt-1 text-sm text-surface-500">{messages.length} messages loaded</p>
	</div>

	<!-- Search / Filter -->
	<div class="card p-4">
		<div class="flex flex-col sm:flex-row gap-3">
			<div class="relative flex-1">
				<Search class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-surface-500" />
				<input
					type="text"
					bind:value={searchQuery}
					onkeydown={(e) => e.key === 'Enter' && doSearch()}
					class="input pl-10"
					placeholder="Search message content…"
				/>
			</div>
			<div class="relative w-full sm:w-64">
				<input
					type="text"
					bind:value={playerFilter}
					oninput={searchPlayers}
					onfocus={() => { if (playerResults.length) showPlayerDropdown = true; }}
					class="input"
					placeholder={selectedPlayer ? '' : 'Filter by player…'}
				/>
				{#if selectedPlayer}
					<button class="absolute right-2 top-1/2 -translate-y-1/2 text-surface-400 hover:text-surface-200 text-xs" onclick={clearPlayer}>✕</button>
				{/if}
				{#if showPlayerDropdown && playerResults.length > 0}
					<div class="absolute z-20 mt-1 w-full rounded border border-surface-700 bg-surface-900 shadow-lg max-h-48 overflow-y-auto">
						{#each playerResults as p}
							<button
								class="block w-full px-3 py-2 text-left text-sm hover:bg-surface-800 transition-colors"
								onclick={() => selectPlayer(p)}
							>
								{stripColors(p.name)} <span class="text-surface-500 text-xs">#{p.id}</span>
							</button>
						{/each}
					</div>
				{/if}
			</div>
			<button class="btn-primary btn-sm whitespace-nowrap" onclick={doSearch}>Search</button>
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
		</div>
	{:else}
		<div class="card overflow-hidden">
			<table class="w-full text-sm">
				<thead>
					<tr class="border-b border-surface-800 text-left text-xs font-medium uppercase tracking-wider text-surface-500">
						<th class="px-5 py-3 w-36">Time</th>
						<th class="px-5 py-3 w-40">Player</th>
						<th class="px-5 py-3 w-20">Channel</th>
						<th class="px-5 py-3">Message</th>
					</tr>
				</thead>
				<tbody class="divide-y divide-surface-800/50">
					{#each messages as msg}
						<tr class="hover:bg-surface-800/30 transition-colors">
							<td class="px-5 py-3 text-xs text-surface-500 whitespace-nowrap">{timeAgo(msg.time_add)}</td>
							<td class="px-5 py-3">
								<a href="/players/{msg.client_id}" class="text-surface-200 hover:text-accent transition-colors">
									{stripColors(msg.client_name)}
								</a>
							</td>
							<td class="px-5 py-3">
								<span class={channelBadge(msg.channel)}>{msg.channel}</span>
							</td>
							<td class="px-5 py-3 text-surface-300 break-all">{msg.message}</td>
						</tr>
					{/each}
				</tbody>
			</table>
			{#if messages.length === 0}
				<div class="px-5 py-10 text-center text-sm text-surface-500">No chat messages found</div>
			{/if}
		</div>

		{#if hasMore && messages.length > 0}
			<div class="flex justify-center">
				<button
					class="btn-secondary btn-sm"
					disabled={loadingMore}
					onclick={() => loadMessages(true)}
				>
					{#if loadingMore}
						<div class="h-3.5 w-3.5 animate-spin rounded-full border-2 border-surface-400/20 border-t-surface-400"></div>
					{:else}
						<ChevronDown class="h-3.5 w-3.5" />
					{/if}
					Load More
				</button>
			</div>
		{/if}
	{/if}
</div>
