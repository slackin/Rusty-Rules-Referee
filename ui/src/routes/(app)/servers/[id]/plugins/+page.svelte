<script>
	import { api } from '$lib/api.svelte.js';
	import { page } from '$app/stores';

	let serverId = $derived(Number($page.params.id));
	let plugins = $state([]);
	let error = $state('');
	let loading = $state(false);
	let editing = $state(null); // plugin.name currently being edited
	let editText = $state('');
	let saveMsg = $state('');

	async function load() {
		loading = true;
		error = '';
		try {
			const r = await api.serverListPlugins(serverId);
			plugins = r.plugins || [];
		} catch (e) { error = e.message; }
		finally { loading = false; }
	}
	$effect(() => { load(); });

	async function toggle(p) {
		try {
			await api.serverUpdatePlugin(serverId, p.name, { enabled: !p.enabled });
			await load();
		} catch (e) { error = e.message; }
	}

	function beginEdit(p) {
		editing = p.name;
		try { editText = JSON.stringify(p.settings ?? {}, null, 2); }
		catch (_) { editText = '{}'; }
		saveMsg = '';
	}

	async function saveEdit() {
		saveMsg = '';
		let parsed;
		try { parsed = JSON.parse(editText || '{}'); }
		catch (e) { saveMsg = 'Invalid JSON: ' + e.message; return; }
		try {
			await api.serverUpdatePlugin(serverId, editing, { settings: parsed });
			saveMsg = 'Saved — client will restart on next heartbeat.';
			editing = null;
			await load();
		} catch (e) { saveMsg = e.message; }
	}
</script>

<h2 class="text-xl font-semibold mb-3">Plugins</h2>
<p class="text-sm text-gray-500 mb-4">
	Changes are saved to the master and pushed to this server's bot on its next heartbeat.
	The bot will restart automatically to apply new plugin settings.
</p>

{#if error}<div class="text-red-600 mb-2">{error}</div>{/if}
{#if saveMsg}<div class="text-blue-600 mb-2">{saveMsg}</div>{/if}

<table class="min-w-full text-sm">
	<thead class="bg-gray-50">
		<tr>
			<th class="p-2 text-left">Plugin</th>
			<th class="p-2">Enabled</th>
			<th class="p-2 text-left">Settings</th>
			<th class="p-2"></th>
		</tr>
	</thead>
	<tbody>
		{#each plugins as p}
			<tr class="border-t align-top">
				<td class="p-2 font-mono">{p.name}</td>
				<td class="p-2 text-center">
					<input type="checkbox" checked={p.enabled} onchange={() => toggle(p)} />
				</td>
				<td class="p-2">
					{#if editing === p.name}
						<textarea class="textarea textarea-bordered w-full font-mono" rows="8" bind:value={editText}></textarea>
					{:else}
						<pre class="text-xs bg-gray-50 p-2 rounded max-h-24 overflow-auto">{JSON.stringify(p.settings ?? {}, null, 2)}</pre>
					{/if}
				</td>
				<td class="p-2">
					{#if editing === p.name}
						<button class="btn btn-sm btn-primary" onclick={saveEdit}>Save</button>
						<button class="btn btn-sm" onclick={() => editing = null}>Cancel</button>
					{:else}
						<button class="btn btn-sm" onclick={() => beginEdit(p)}>Edit</button>
					{/if}
				</td>
			</tr>
		{:else}
			<tr><td colspan="4" class="p-4 text-center text-gray-400">{loading ? 'Loading…' : 'No plugin config has been pushed yet'}</td></tr>
		{/each}
	</tbody>
</table>
