<script>
	import { api } from '$lib/api.svelte.js';
	import { page } from '$app/stores';

	let serverId = $derived(Number($page.params.id));
	let rows = $state([]);
	let error = $state('');
	let loading = $state(false);

	async function load() {
		loading = true;
		error = '';
		try {
			const r = await api.serverPenalties(serverId, 100, 0);
			rows = r.penalties || [];
		} catch (e) { error = e.message; }
		finally { loading = false; }
	}
	$effect(() => { load(); });
</script>

<h2 class="text-xl font-semibold mb-3">Penalties (this server)</h2>
{#if error}<div class="text-red-400 mb-2">{error}</div>{/if}

<div class="card overflow-hidden">
	<table class="w-full text-sm">
		<thead>
			<tr class="border-b border-surface-800 text-left text-xs font-medium uppercase tracking-wider text-surface-500">
				<th class="px-4 py-3">Time</th>
				<th class="px-4 py-3">Type</th>
				<th class="px-4 py-3">Client</th>
				<th class="px-4 py-3">Reason</th>
				<th class="px-4 py-3">Expires</th>
			</tr>
		</thead>
		<tbody class="divide-y divide-surface-800/50">
			{#each rows as p}
				<tr class="hover:bg-surface-800/30 transition-colors">
					<td class="px-4 py-2 text-surface-300">{new Date(p.time_add).toLocaleString()}</td>
					<td class="px-4 py-2 text-surface-200">{p.penalty_type}</td>
					<td class="px-4 py-2 text-surface-300">#{p.client_id}</td>
					<td class="px-4 py-2 text-surface-400">{p.reason}</td>
					<td class="px-4 py-2 text-surface-400">{p.time_expire ? new Date(p.time_expire).toLocaleString() : '—'}</td>
				</tr>
			{:else}
				<tr><td colspan="5" class="px-4 py-6 text-center text-surface-500">{loading ? 'Loading…' : 'No penalties recorded for this server yet'}</td></tr>
			{/each}
		</tbody>
	</table>
</div>
