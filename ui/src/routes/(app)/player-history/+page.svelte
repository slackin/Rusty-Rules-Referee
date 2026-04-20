<script>
	import { onMount } from 'svelte';
	import { api } from '$lib/api.svelte.js';
	import { stripColors, timeAgo } from '$lib/utils.js';
	import { Search, ChevronLeft, ChevronRight, ArrowUpDown, ArrowUp, ArrowDown, ExternalLink } from 'lucide-svelte';

	let clients = $state([]);
	let total = $state(0);
	let loading = $state(true);
	let search = $state('');
	let offset = $state(0);
	let sortBy = $state('last_visit');
	let order = $state('desc');
	const limit = 25;

	let searchTimeout;

	onMount(() => { loadPage(); });

	async function loadPage() {
		loading = true;
		try {
			const res = await api.allClients({ limit, offset, search, sortBy, order });
			clients = res.clients;
			total = res.total;
		} catch (e) {
			console.error(e);
		}
		loading = false;
	}

	function onSearch() {
		clearTimeout(searchTimeout);
		searchTimeout = setTimeout(() => {
			offset = 0;
			loadPage();
		}, 300);
	}

	function nextPage() { offset += limit; loadPage(); }
	function prevPage() { if (offset >= limit) { offset -= limit; loadPage(); } }

	function toggleSort(col) {
		if (sortBy === col) {
			order = order === 'desc' ? 'asc' : 'desc';
		} else {
			sortBy = col;
			order = col === 'name' ? 'asc' : 'desc';
		}
		offset = 0;
		loadPage();
	}

	let totalPages = $derived(Math.max(1, Math.ceil(total / limit)));
	let currentPage = $derived(Math.floor(offset / limit) + 1);
</script>

<div class="space-y-6 animate-fade-in">
	<div>
		<h1 class="text-2xl font-semibold">Player History</h1>
		<p class="mt-1 text-sm text-surface-500">All players who have connected to the server ({total.toLocaleString()} total)</p>
	</div>

	<div class="card p-4">
		<div class="relative">
			<Search class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-surface-500" />
			<input type="text" bind:value={search} oninput={onSearch} class="input pl-10" placeholder="Search by name, GUID, or IP…" />
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
		</div>
	{:else}
		<div class="card overflow-hidden">
			<div class="overflow-x-auto">
				<table class="w-full text-sm">
					<thead>
						<tr class="border-b border-surface-800 text-left text-xs font-medium uppercase tracking-wider text-surface-500">
							<th class="px-5 py-3 w-16">ID</th>
							<th class="px-5 py-3">
								<button class="flex items-center gap-1 hover:text-surface-200 transition-colors" onclick={() => toggleSort('name')}>
									Name
									{#if sortBy === 'name'}
										{#if order === 'asc'}<ArrowUp class="h-3 w-3" />{:else}<ArrowDown class="h-3 w-3" />{/if}
									{:else}
										<ArrowUpDown class="h-3 w-3 opacity-40" />
									{/if}
								</button>
							</th>
							<th class="px-5 py-3">IP</th>
							<th class="px-5 py-3">Group</th>
							<th class="px-5 py-3">
								<button class="flex items-center gap-1 hover:text-surface-200 transition-colors" onclick={() => toggleSort('time_add')}>
									First Seen
									{#if sortBy === 'time_add'}
										{#if order === 'asc'}<ArrowUp class="h-3 w-3" />{:else}<ArrowDown class="h-3 w-3" />{/if}
									{:else}
										<ArrowUpDown class="h-3 w-3 opacity-40" />
									{/if}
								</button>
							</th>
							<th class="px-5 py-3">
								<button class="flex items-center gap-1 hover:text-surface-200 transition-colors" onclick={() => toggleSort('last_visit')}>
									Last Visit
									{#if sortBy === 'last_visit'}
										{#if order === 'asc'}<ArrowUp class="h-3 w-3" />{:else}<ArrowDown class="h-3 w-3" />{/if}
									{:else}
										<ArrowUpDown class="h-3 w-3 opacity-40" />
									{/if}
								</button>
							</th>
							<th class="px-5 py-3">Status</th>
							<th class="px-5 py-3 w-16"></th>
						</tr>
					</thead>
					<tbody class="divide-y divide-surface-800/50">
						{#each clients as client}
							<tr class="hover:bg-surface-800/30 transition-colors cursor-pointer" onclick={() => window.location.href = `/players/${client.id}`}>
								<td class="px-5 py-3 text-surface-500 font-mono text-xs">@{client.id}</td>
								<td class="px-5 py-3 text-surface-200 font-medium">
									<div class="flex items-center gap-1.5">
										<span>{stripColors(client.current_name || client.name)}</span>
										{#if client.auth}
											<span class="text-[10px] uppercase font-bold px-1.5 py-0.5 rounded bg-purple-500/15 text-purple-400">{client.auth}</span>
										{/if}
									</div>
									{#if client.current_name && stripColors(client.current_name) !== stripColors(client.name)}
										<div class="text-[10px] text-surface-500">aka {stripColors(client.name)}</div>
									{/if}
								</td>
								<td class="px-5 py-3 text-surface-500 font-mono text-xs">{client.ip ?? '—'}</td>
								<td class="px-5 py-3">
									{#if client.group_name}
										<span class="badge-blue">{client.group_name}</span>
									{:else}
										<span class="text-surface-600">Guest</span>
									{/if}
								</td>
								<td class="px-5 py-3 text-xs text-surface-500">{client.time_add ? timeAgo(client.time_add) : '—'}</td>
								<td class="px-5 py-3 text-xs text-surface-500">{client.last_visit ? timeAgo(client.last_visit) : '—'}</td>
								<td class="px-5 py-3">
									{#if client.online}
										<span class="inline-flex items-center gap-1.5">
											<span class="h-2 w-2 rounded-full bg-green-500 animate-pulse-soft"></span>
											<span class="text-xs text-green-400">Online</span>
										</span>
									{:else}
										<span class="text-xs text-surface-600">Offline</span>
									{/if}
								</td>
								<td class="px-5 py-3">
									<a href="/players/{client.id}" class="text-accent hover:text-accent/80 transition-colors" onclick={(e) => e.stopPropagation()}>
										<ExternalLink class="h-4 w-4" />
									</a>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
			{#if clients.length === 0}
				<div class="px-5 py-10 text-center text-sm text-surface-500">
					{search ? 'No players match your search' : 'No players found'}
				</div>
			{/if}
		</div>

		<!-- Pagination -->
		<div class="flex items-center justify-between">
			<button class="btn-ghost btn-sm flex items-center gap-1" disabled={offset === 0} onclick={prevPage}>
				<ChevronLeft class="h-4 w-4" /> Previous
			</button>
			<span class="text-xs text-surface-500">Page {currentPage} of {totalPages}</span>
			<button class="btn-ghost btn-sm flex items-center gap-1" disabled={clients.length < limit} onclick={nextPage}>
				Next <ChevronRight class="h-4 w-4" />
			</button>
		</div>
	{/if}
</div>
