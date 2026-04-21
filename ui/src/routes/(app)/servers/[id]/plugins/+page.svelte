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
<p class="text-sm text-surface-500 mb-4">
	Changes are saved to the master and pushed to this server's bot on its next heartbeat.
	The bot will restart automatically to apply new plugin settings.
</p>

{#if error}<div class="text-red-400 mb-2">{error}</div>{/if}
{#if saveMsg}<div class="text-accent mb-2">{saveMsg}</div>{/if}

<div class="card overflow-hidden">
	<table class="w-full text-sm">
		<thead>
			<tr class="border-b border-surface-800 text-left text-xs font-medium uppercase tracking-wider text-surface-500">
				<th class="px-4 py-3">Plugin</th>
				<th class="px-4 py-3 text-center">Enabled</th>
				<th class="px-4 py-3">Settings</th>
				<th class="px-4 py-3"></th>
			</tr>
		</thead>
		<tbody class="divide-y divide-surface-800/50">
			{#each plugins as p}
				<tr class="align-top hover:bg-surface-800/30 transition-colors">
					<td class="px-4 py-3 font-mono text-surface-200">{p.name}</td>
					<td class="px-4 py-3 text-center">
						<input type="checkbox" checked={p.enabled} onchange={() => toggle(p)} />
					</td>
					<td class="px-4 py-3">
						{#if editing === p.name}
							<textarea class="input font-mono" rows="8" bind:value={editText}></textarea>
						{:else}
							<pre class="text-xs bg-surface-950/50 border border-surface-800 text-surface-300 p-2 rounded max-h-24 overflow-auto">{JSON.stringify(p.settings ?? {}, null, 2)}</pre>
						{/if}
					</td>
					<td class="px-4 py-3">
						{#if editing === p.name}
							<button class="btn btn-primary btn-sm" onclick={saveEdit}>Save</button>
							<button class="btn btn-secondary btn-sm ml-1" onclick={() => editing = null}>Cancel</button>
						{:else}
							<button class="btn btn-secondary btn-sm" onclick={() => beginEdit(p)}>Edit</button>
						{/if}
					</td>
				</tr>
			{:else}
				<tr><td colspan="4" class="px-4 py-6 text-center text-surface-500">{loading ? 'Loading…' : 'No plugin config has been pushed yet'}</td></tr>
			{/each}
		</tbody>
	</table>
</div>
