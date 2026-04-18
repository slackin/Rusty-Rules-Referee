<script>
	import { onMount } from 'svelte';
	import { api } from '$lib/api.js';
	import { stripColors, timeAgo, formatDuration } from '$lib/utils.js';
	import { Search, XCircle } from 'lucide-svelte';

	let penalties = $state([]);
	let loading = $state(true);
	let filter = $state('');

	onMount(async () => {
		try {
			penalties = await api.penalties('limit=100');
		} catch (e) {
			console.error(e);
		}
		loading = false;
	});

	async function disable(id) {
		if (!confirm('Disable this penalty?')) return;
		try {
			await api.disablePenalty(id);
			penalties = penalties.map((p) => (p.id === id ? { ...p, inactive: 1 } : p));
		} catch (e) {
			alert(e.message);
		}
	}

	let filtered = $derived(
		filter
			? penalties.filter((p) => p.type?.toLowerCase().includes(filter.toLowerCase()) || p.reason?.toLowerCase().includes(filter.toLowerCase()))
			: penalties
	);
</script>

<div class="space-y-6 animate-fade-in">
	<div>
		<h1 class="text-2xl font-semibold">Penalties</h1>
		<p class="mt-1 text-sm text-surface-500">{penalties.length} total penalties</p>
	</div>

	<div class="card p-4">
		<div class="relative">
			<Search class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-surface-500" />
			<input type="text" bind:value={filter} class="input pl-10" placeholder="Filter by type or reason…" />
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
						<th class="px-5 py-3">Type</th>
						<th class="px-5 py-3">Player</th>
						<th class="px-5 py-3">Reason</th>
						<th class="px-5 py-3">Duration</th>
						<th class="px-5 py-3">Date</th>
						<th class="px-5 py-3">Status</th>
						<th class="px-5 py-3 text-right">Actions</th>
					</tr>
				</thead>
				<tbody class="divide-y divide-surface-800/50">
					{#each filtered as pen}
						<tr class="hover:bg-surface-800/30 transition-colors {pen.inactive ? 'opacity-50' : ''}">
							<td class="px-5 py-3">
								<span class="{pen.type === 'Ban' || pen.type === 'TempBan' ? 'badge-red' : 'badge-yellow'}">
									{pen.type}
								</span>
							</td>
							<td class="px-5 py-3">
								<a href="/players/{pen.client_id}" class="text-surface-200 hover:text-accent">{pen.client_name ? stripColors(pen.client_name) : `#${pen.client_id}`}</a>
							</td>
							<td class="px-5 py-3 text-surface-400 max-w-xs truncate">{pen.reason ?? '—'}</td>
							<td class="px-5 py-3 text-surface-500">{formatDuration(pen.duration)}</td>
							<td class="px-5 py-3 text-xs text-surface-500">{timeAgo(pen.time_add)}</td>
							<td class="px-5 py-3">
								{#if pen.inactive}
									<span class="badge-gray">Inactive</span>
								{:else}
									<span class="badge-green">Active</span>
								{/if}
							</td>
							<td class="px-5 py-3 text-right">
								{#if !pen.inactive}
									<button class="btn-ghost btn-sm" title="Disable penalty" onclick={() => disable(pen.id)}>
										<XCircle class="h-3.5 w-3.5" />
									</button>
								{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
			{#if filtered.length === 0}
				<div class="px-5 py-10 text-center text-sm text-surface-500">No penalties found</div>
			{/if}
		</div>
	{/if}
</div>
