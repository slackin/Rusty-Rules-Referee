<script>
	import { api } from '$lib/api.svelte.js';
	import { page } from '$app/stores';

	let serverId = $derived(Number($page.params.id));
	let mapcyclePath = $state('');
	let maps = $state([]);
	let text = $state('');
	let loading = $state(false);
	let saving = $state(false);
	let error = $state('');
	let msg = $state('');

	async function load() {
		loading = true; error = ''; msg = '';
		try {
			const r = await api.serverGetMapcycle(serverId);
			const d = r?.data ?? r?.Mapcycle ?? {};
			mapcyclePath = d.path || '';
			maps = d.maps || [];
			text = maps.join('\n');
		} catch (e) { error = e.message; }
		finally { loading = false; }
	}
	$effect(() => { load(); });

	async function save() {
		saving = true; msg = '';
		const list = text
			.split(/\r?\n/)
			.map(s => s.trim())
			.filter(Boolean);
		try {
			const r = await api.serverSetMapcycle(serverId, list);
			msg = (r?.data ?? r?.Ok)?.message || 'Saved';
			await load();
		} catch (e) { msg = e.message; }
		finally { saving = false; }
	}
</script>

<h2 class="text-xl font-semibold mb-3">Mapcycle</h2>
<p class="text-sm text-surface-500 mb-3">File: <code class="text-surface-300">{mapcyclePath}</code> — one map per line.</p>
{#if error}<div class="text-red-400 mb-2">{error}</div>{/if}
{#if msg}<div class="text-accent mb-2">{msg}</div>{/if}

<textarea class="input font-mono" rows="20" bind:value={text} disabled={loading}></textarea>
<div class="mt-3">
	<button class="btn btn-primary" onclick={save} disabled={saving || loading}>
		{saving ? 'Saving…' : 'Save Mapcycle'}
	</button>
	<button class="btn btn-secondary ml-2" onclick={load} disabled={loading}>Reload</button>
</div>
