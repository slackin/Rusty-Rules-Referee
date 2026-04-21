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
			const r = await api.serverChat(serverId, 200);
			rows = r.messages || [];
		} catch (e) { error = e.message; }
		finally { loading = false; }
	}
	$effect(() => { load(); });
	$effect(() => { const h = setInterval(load, 15000); return () => clearInterval(h); });
</script>

<h2 class="text-xl font-semibold mb-3">Chat (this server)</h2>
{#if error}<div class="text-red-600 mb-2">{error}</div>{/if}

<div class="bg-gray-50 rounded p-2 text-sm font-mono max-h-[70vh] overflow-y-auto">
	{#each rows as m}
		<div class="py-0.5">
			<span class="text-gray-400">{new Date(m.time_add).toLocaleTimeString()}</span>
			<span class="font-bold ml-2">{m.client_name || `#${m.client_id}`}:</span>
			<span class="ml-1">{m.message}</span>
		</div>
	{:else}
		<div class="text-gray-400 p-2">{loading ? 'Loading…' : 'No chat history yet'}</div>
	{/each}
</div>
