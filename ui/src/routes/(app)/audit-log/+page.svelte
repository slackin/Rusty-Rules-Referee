<script>
	import { onMount } from 'svelte';
	import { api } from '$lib/api.svelte.js';
	import { timeAgo } from '$lib/utils.js';
	import { Search, ChevronLeft, ChevronRight } from 'lucide-svelte';

	let entries = $state([]);
	let loading = $state(true);
	let filter = $state('');
	let offset = $state(0);
	const limit = 50;

	onMount(() => { loadPage(); });

	async function loadPage() {
		loading = true;
		try {
			entries = await api.auditLog(limit, offset);
		} catch (e) {
			console.error(e);
		}
		loading = false;
	}

	function nextPage() { offset += limit; loadPage(); }
	function prevPage() { if (offset >= limit) { offset -= limit; loadPage(); } }

	let filtered = $derived(
		filter
			? entries.filter(e =>
				e.action?.toLowerCase().includes(filter.toLowerCase()) ||
				e.admin_username?.toLowerCase().includes(filter.toLowerCase()) ||
				e.detail?.toLowerCase().includes(filter.toLowerCase())
			)
			: entries
	);
</script>

<div class="space-y-6 animate-fade-in">
	<div>
		<h1 class="text-2xl font-semibold">Audit Log</h1>
		<p class="mt-1 text-sm text-surface-500">Admin action history</p>
	</div>

	<div class="card p-4">
		<div class="relative">
			<Search class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-surface-500" />
			<input type="text" bind:value={filter} class="input pl-10" placeholder="Filter by action, user, or detail…" />
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
						<th class="px-5 py-3">Admin</th>
						<th class="px-5 py-3">Action</th>
						<th class="px-5 py-3">Detail</th>
						<th class="px-5 py-3">IP</th>
						<th class="px-5 py-3">Date</th>
					</tr>
				</thead>
				<tbody class="divide-y divide-surface-800/50">
					{#each filtered as entry}
						<tr class="hover:bg-surface-800/30 transition-colors">
							<td class="px-5 py-3 text-surface-200">{entry.admin_username}</td>
							<td class="px-5 py-3"><span class="badge-blue">{entry.action}</span></td>
							<td class="px-5 py-3 text-surface-400 max-w-sm truncate">{entry.detail ?? '—'}</td>
							<td class="px-5 py-3 text-surface-500 font-mono text-xs">{entry.ip_address ?? '—'}</td>
							<td class="px-5 py-3 text-xs text-surface-500">{timeAgo(entry.created_at)}</td>
						</tr>
					{/each}
				</tbody>
			</table>
			{#if filtered.length === 0}
				<div class="px-5 py-10 text-center text-sm text-surface-500">No audit log entries found</div>
			{/if}
		</div>

		<!-- Pagination -->
		<div class="flex items-center justify-between">
			<button class="btn-ghost btn-sm flex items-center gap-1" disabled={offset === 0} onclick={prevPage}>
				<ChevronLeft class="h-4 w-4" /> Previous
			</button>
			<span class="text-xs text-surface-500">Page {Math.floor(offset / limit) + 1}</span>
			<button class="btn-ghost btn-sm flex items-center gap-1" disabled={entries.length < limit} onclick={nextPage}>
				Next <ChevronRight class="h-4 w-4" />
			</button>
		</div>
	{/if}
</div>
