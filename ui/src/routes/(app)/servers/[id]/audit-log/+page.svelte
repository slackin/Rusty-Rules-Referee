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
			const r = await api.serverAuditLog(serverId, 200);
			rows = r.entries || [];
		} catch (e) { error = e.message; }
		finally { loading = false; }
	}
	$effect(() => { load(); });
</script>

<h2 class="text-xl font-semibold mb-3">Audit Log (this server)</h2>
{#if error}<div class="text-red-600 mb-2">{error}</div>{/if}

<table class="min-w-full text-sm">
	<thead class="bg-gray-50">
		<tr>
			<th class="p-2 text-left">Time</th>
			<th class="p-2 text-left">Admin</th>
			<th class="p-2 text-left">Action</th>
			<th class="p-2 text-left">Detail</th>
			<th class="p-2 text-left">IP</th>
		</tr>
	</thead>
	<tbody>
		{#each rows as e}
			<tr class="border-t">
				<td class="p-2">{new Date(e.created_at).toLocaleString()}</td>
				<td class="p-2">#{e.admin_user_id ?? '—'}</td>
				<td class="p-2 font-mono">{e.action}</td>
				<td class="p-2">{e.detail}</td>
				<td class="p-2 font-mono">{e.ip_address || '—'}</td>
			</tr>
		{:else}
			<tr><td colspan="5" class="p-4 text-center text-gray-400">{loading ? 'Loading…' : 'No audit entries for this server'}</td></tr>
		{/each}
	</tbody>
</table>
