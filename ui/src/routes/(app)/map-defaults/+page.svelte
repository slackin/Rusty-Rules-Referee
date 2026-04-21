<script>
	import { api } from '$lib/api.svelte.js';
	import { isMaster } from '$lib/mode.svelte.js';
	import MapConfigEditor from '$lib/mapconfig/MapConfigEditor.svelte';
	import { Save, Search, Share2, Trash2, Plus } from 'lucide-svelte';

	let defaults = $state([]);
	let loading = $state(false);
	let selectedName = $state('');
	let editing = $state(null);
	let original = $state(null);
	let saving = $state(false);
	let propagating = $state(false);
	let overwriteUser = $state(false);
	let error = $state('');
	let msg = $state('');
	let search = $state('');
	let newName = $state('');

	let filtered = $derived(
		search.trim()
			? defaults.filter((d) => d.map_name.toLowerCase().includes(search.trim().toLowerCase()))
			: defaults,
	);

	let dirty = $derived(
		editing && original ? JSON.stringify(editing) !== JSON.stringify(original) : false,
	);

	function blankDefault(name) {
		return {
			map_name: name,
			gametype: '',
			capturelimit: null,
			timelimit: null,
			fraglimit: null,
			g_gear: '',
			g_gravity: null,
			g_friendlyfire: null,
			g_followstrict: null,
			g_waverespawns: null,
			g_bombdefusetime: null,
			g_bombexplodetime: null,
			g_swaproles: null,
			g_maxrounds: null,
			g_matchmode: null,
			g_respawndelay: null,
			startmessage: '',
			skiprandom: 0,
			bot: 0,
			custom_commands: '',
			supported_gametypes: '',
			default_gametype: null,
			g_suddendeath: null,
			g_teamdamage: null,
			source: 'default_seed',
			created_at: new Date(0).toISOString(),
			updated_at: new Date().toISOString(),
		};
	}

	async function load() {
		loading = true; error = '';
		try {
			const r = await api.mapConfigDefaults();
			defaults = r?.defaults || [];
			if (!selectedName && defaults.length) await select(defaults[0].map_name);
		} catch (e) { error = e.message || String(e); }
		finally { loading = false; }
	}

	async function select(name) {
		selectedName = name; msg = ''; error = '';
		try {
			const r = await api.mapConfigDefault(name);
			editing = structuredClone(r);
			original = structuredClone(r);
		} catch (e) { error = e.message || String(e); }
	}

	async function save() {
		if (!editing) return;
		saving = true; msg = ''; error = '';
		try {
			await api.saveMapConfigDefault(editing.map_name, editing);
			original = structuredClone(editing);
			const idx = defaults.findIndex((d) => d.map_name === editing.map_name);
			if (idx >= 0) defaults[idx] = structuredClone(editing);
			else defaults = [...defaults, structuredClone(editing)];
			msg = 'Saved.';
		} catch (e) { error = e.message || String(e); }
		finally { saving = false; }
	}

	async function create() {
		const n = newName.trim();
		if (!n) return;
		if (defaults.some((d) => d.map_name === n)) { error = 'Already exists'; return; }
		editing = blankDefault(n);
		original = null;
		selectedName = n;
		newName = '';
	}

	async function removeCurrent() {
		if (!editing) return;
		if (!confirm(`Delete global default for ${editing.map_name}? Servers keep their current rows.`)) return;
		try {
			await api.deleteMapConfigDefault(editing.map_name);
			defaults = defaults.filter((d) => d.map_name !== editing.map_name);
			editing = null; original = null; selectedName = '';
		} catch (e) { error = e.message || String(e); }
	}

	async function propagate() {
		if (!editing) return;
		if (!confirm(`Propagate '${editing.map_name}' default to every server?${overwriteUser ? ' This WILL overwrite admin-edited rows.' : ''}`)) return;
		propagating = true; msg = ''; error = '';
		try {
			const r = await api.propagateMapConfigDefault(editing.map_name, overwriteUser);
			msg = `Propagated. master_updated=${r.master_updated}, clients_sent=${r.clients_sent}, skipped=${r.clients_skipped}, failed=${r.clients_failed}.`;
		} catch (e) { error = e.message || String(e); }
		finally { propagating = false; }
	}

	$effect(() => { load(); });
</script>

{#if !isMaster()}
	<div class="rounded border border-amber-500/30 bg-amber-500/10 p-4 text-sm text-amber-300">
		Map Defaults are only available in master mode.
	</div>
{:else}
<div class="flex h-full min-h-[70vh] gap-4">
	<aside class="flex w-72 shrink-0 flex-col rounded-lg border border-zinc-800 bg-zinc-900/40">
		<div class="border-b border-zinc-800 p-3 space-y-2">
			<div class="relative">
				<Search class="pointer-events-none absolute left-2 top-2.5 h-3.5 w-3.5 text-zinc-500" />
				<input type="text" class="w-full rounded border border-zinc-700 bg-zinc-950 py-1.5 pl-7 pr-2 text-sm"
					placeholder="Search defaults…" bind:value={search} />
			</div>
			<div class="flex gap-1">
				<input type="text" class="flex-1 rounded border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-sm"
					placeholder="new map name" bind:value={newName} />
				<button class="btn-secondary text-xs" onclick={create} disabled={!newName.trim()}>
					<Plus class="h-3 w-3" />
				</button>
			</div>
			<div class="text-xs text-zinc-500">{filtered.length} / {defaults.length}</div>
		</div>
		<div class="flex-1 overflow-y-auto">
			{#if loading}
				<div class="p-3 text-xs text-zinc-500">Loading…</div>
			{:else}
				{#each filtered as d}
					<button type="button"
						class="block w-full truncate border-l-2 px-3 py-2 text-left text-sm transition {selectedName === d.map_name
							? 'border-blue-500 bg-blue-500/10 text-blue-200'
							: 'border-transparent text-zinc-300 hover:bg-zinc-800/50'}"
						onclick={() => select(d.map_name)}>
						{d.map_name}
					</button>
				{/each}
			{/if}
		</div>
	</aside>

	<section class="flex min-w-0 flex-1 flex-col">
		{#if !editing}
			<div class="flex flex-1 items-center justify-center text-sm text-zinc-500">
				Select or create a default on the left.
			</div>
		{:else}
			<div class="mb-3 flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-900/40 p-3">
				<div class="min-w-0 flex-1">
					<div class="truncate text-base font-semibold text-zinc-100">{editing.map_name}</div>
					<div class="text-xs text-zinc-500">global default template</div>
				</div>
				<label class="flex items-center gap-1 text-xs text-zinc-400">
					<input type="checkbox" bind:checked={overwriteUser} class="accent-blue-500" />
					Overwrite admin-edited rows
				</label>
				<button class="btn-secondary flex items-center gap-1 text-sm" onclick={propagate} disabled={propagating || dirty}>
					<Share2 class="h-4 w-4" />
					{propagating ? '…' : 'Propagate to all servers'}
				</button>
				<button class="btn-secondary flex items-center gap-1 text-sm text-red-400" onclick={removeCurrent}>
					<Trash2 class="h-4 w-4" />
					Delete
				</button>
			</div>

			{#if error}<div class="mb-2 rounded border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-300">{error}</div>{/if}
			{#if msg}<div class="mb-2 rounded border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-sm text-emerald-300">{msg}</div>{/if}

			<div class="flex-1 overflow-y-auto pr-1">
				<MapConfigEditor bind:config={editing} disabled={saving} />
			</div>

			<div class="mt-3 flex justify-end gap-2 border-t border-zinc-800 pt-3">
				<button class="btn flex items-center gap-1 text-sm" onclick={save} disabled={!dirty || saving}>
					<Save class="h-4 w-4" />
					{saving ? 'Saving…' : 'Save default'}
				</button>
			</div>
		{/if}
	</section>
</div>
{/if}
