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
{#if error}<div class="text-red-600 mb-2">{error}</div>{/if}

<table class="min-w-full text-sm">
	<thead class="bg-gray-50">
		<tr>
			<th class="p-2 text-left">Time</th>
			<th class="p-2 text-left">Type</th>
			<th class="p-2 text-left">Client</th>
			<th class="p-2 text-left">Reason</th>
			<th class="p-2 text-left">Expires</th>
		</tr>
	</thead>
	<tbody>
		{#each rows as p}
			<tr class="border-t">
				<td class="p-2">{new Date(p.time_add).toLocaleString()}</td>
				<td class="p-2">{p.penalty_type}</td>
				<td class="p-2">#{p.client_id}</td>
				<td class="p-2">{p.reason}</td>
				<td class="p-2">{p.time_expire ? new Date(p.time_expire).toLocaleString() : '—'}</td>
			</tr>
		{:else}
			<tr><td colspan="5" class="p-4 text-center text-gray-400">{loading ? 'Loading…' : 'No penalties recorded for this server yet'}</td></tr>
		{/each}
	</tbody>
</table>
