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
{#if error}<div class="text-red-400 mb-2">{error}</div>{/if}

<div class="card overflow-hidden">
	<table class="w-full text-sm">
		<thead>
			<tr class="border-b border-surface-800 text-left text-xs font-medium uppercase tracking-wider text-surface-500">
				<th class="px-4 py-3">Time</th>
				<th class="px-4 py-3">Admin</th>
				<th class="px-4 py-3">Action</th>
				<th class="px-4 py-3">Detail</th>
				<th class="px-4 py-3">IP</th>
			</tr>
		</thead>
		<tbody class="divide-y divide-surface-800/50">
			{#each rows as e}
				<tr class="hover:bg-surface-800/30 transition-colors">
					<td class="px-4 py-2 text-surface-300">{new Date(e.created_at).toLocaleString()}</td>
					<td class="px-4 py-2 text-surface-300">#{e.admin_user_id ?? '—'}</td>
					<td class="px-4 py-2 font-mono text-surface-200">{e.action}</td>
					<td class="px-4 py-2 text-surface-400">{e.detail}</td>
					<td class="px-4 py-2 font-mono text-surface-400">{e.ip_address || '—'}</td>
				</tr>
			{:else}
				<tr><td colspan="5" class="px-4 py-6 text-center text-surface-500">{loading ? 'Loading…' : 'No audit entries for this server'}</td></tr>
			{/each}
		</tbody>
	</table>
</div>
