<script>
	import { api } from '$lib/api.svelte.js';
	import { page } from '$app/stores';

	let serverId = $derived(Number($page.params.id));
	let maps = $state([]);
	let changing = $state('');
	let error = $state('');
	let loading = $state(false);
	let msg = $state('');

	async function load() {
		loading = true; error = '';
		try {
			const r = await api.serverMaps(serverId);
			const list = Array.isArray(r?.maps) ? r.maps : [];
			maps = list.map((m) => (typeof m === 'string' ? m : m.map_name)).filter(Boolean);
		} catch (e) { error = e.message; }
		finally { loading = false; }
	}
	$effect(() => { load(); });

	async function change(m) {
		changing = m;
		msg = '';
		try {
			const r = await api.serverChangeMap(serverId, m);
			msg = (r?.data ?? r?.Ok)?.message || 'Map change requested';
		} catch (e) { msg = e.message; }
		finally { changing = ''; }
	}
</script>

<h2 class="text-xl font-semibold mb-3">Available Maps</h2>
<p class="text-sm text-surface-500 mb-3">
	This view also serves as the map-config landing page. Click a map to change to it now;
	per-map config editing will land here next.
</p>
{#if error}<div class="text-red-400 mb-2">{error}</div>{/if}
{#if msg}<div class="text-accent mb-2">{msg}</div>{/if}

<div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-2">
	{#each maps as m}
		<button class="btn btn-secondary btn-sm" disabled={!!changing} onclick={() => change(m)}>
			{changing === m ? '…' : m}
		</button>
	{:else}
		<div class="text-surface-500">{loading ? 'Loading…' : 'No maps found'}</div>
	{/each}
</div>
