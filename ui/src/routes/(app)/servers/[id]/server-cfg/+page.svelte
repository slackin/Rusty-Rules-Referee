<script>
	import { api } from '$lib/api.svelte.js';
	import { page } from '$app/stores';

	let serverId = $derived(Number($page.params.id));
	let cfgPath = $state('');
	let contents = $state('');
	let loading = $state(false);
	let saving = $state(false);
	let error = $state('');
	let msg = $state('');

	async function load() {
		loading = true; error = ''; msg = '';
		try {
			const r = await api.serverGetServerCfg(serverId);
			cfgPath = r.ServerCfg?.path || '';
			contents = r.ServerCfg?.contents || '';
		} catch (e) { error = e.message; }
		finally { loading = false; }
	}
	$effect(() => { load(); });

	async function save() {
		saving = true; msg = '';
		try {
			const r = await api.serverSaveServerCfg(serverId, cfgPath, contents);
			msg = r.Ok?.message || 'Saved';
		} catch (e) { msg = e.message; }
		finally { saving = false; }
	}
</script>

<h2 class="text-xl font-semibold mb-3">server.cfg Editor</h2>
<p class="text-sm text-gray-500 mb-3">File: <code>{cfgPath}</code></p>
{#if error}<div class="text-red-600 mb-2">{error}</div>{/if}
{#if msg}<div class="text-blue-600 mb-2">{msg}</div>{/if}

<textarea class="textarea textarea-bordered w-full font-mono text-xs" rows="28" bind:value={contents} disabled={loading}></textarea>
<div class="mt-3">
	<button class="btn btn-primary" onclick={save} disabled={saving || loading}>
		{saving ? 'Saving…' : 'Save Changes'}
	</button>
	<button class="btn ml-2" onclick={load} disabled={loading}>Reload</button>
</div>
<p class="text-xs text-gray-400 mt-3">
	Changes take effect on next map load (or <code>exec server.cfg</code> via RCON).
</p>
