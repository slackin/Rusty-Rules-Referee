<script>
	import { api } from '$lib/api.svelte.js';
	import { page } from '$app/stores';
	import { ArrowUp, ArrowDown, X, Plus, RotateCcw, Save, Code, List, GripVertical } from 'lucide-svelte';

	let serverId = $derived(Number($page.params.id));
	let mapcyclePath = $state('');
	/** Current working list (what will be saved). */
	let cycle = $state([]);
	/** Last-loaded list — used for the "Revert" button / dirty check. */
	let original = $state([]);
	/** All maps known to the server (from /maps). */
	let availableMaps = $state([]);
	let availableLoading = $state(false);

	let loading = $state(false);
	let saving = $state(false);
	let error = $state('');
	let msg = $state('');

	/** View mode: 'visual' | 'raw'. */
	let mode = $state('visual');
	/** Raw textarea contents (only used in raw mode). */
	let rawText = $state('');

	/** Filter for the "Add a map" picker. */
	let addFilter = $state('');
	/** Manual map name entry, for maps not in the available list. */
	let manualMap = $state('');

	/** Drag & drop state. */
	let dragFrom = $state(-1);
	let dragOver = $state(-1);

	async function load() {
		loading = true; error = ''; msg = '';
		try {
			const r = await api.serverGetMapcycle(serverId);
			const d = r?.data ?? r?.Mapcycle ?? {};
			mapcyclePath = d.path || '';
			cycle = (d.maps || []).slice();
			original = cycle.slice();
			rawText = cycle.join('\n');
		} catch (e) { error = e.message; }
		finally { loading = false; }
	}

	async function loadAvailable() {
		availableLoading = true;
		try {
			const r = await api.serverMaps(serverId);
			const d = r?.data ?? r?.MapList ?? {};
			availableMaps = (d.maps || []).slice().sort();
		} catch (_) { /* non-fatal; manual add still works */ }
		finally { availableLoading = false; }
	}

	$effect(() => { load(); loadAvailable(); });

	let dirty = $derived.by(() => {
		if (cycle.length !== original.length) return true;
		for (let i = 0; i < cycle.length; i++) if (cycle[i] !== original[i]) return true;
		return false;
	});

	let suggestions = $derived.by(() => {
		const q = addFilter.trim().toLowerCase();
		const existing = new Set(cycle);
		return availableMaps
			.filter(m => !existing.has(m))
			.filter(m => !q || m.toLowerCase().includes(q))
			.slice(0, 60);
	});

	function syncFromRaw() {
		cycle = rawText.split(/\r?\n/).map(s => s.trim()).filter(Boolean);
	}
	function syncToRaw() {
		rawText = cycle.join('\n');
	}

	function switchMode(m) {
		if (m === mode) return;
		if (m === 'raw') syncToRaw();
		else syncFromRaw();
		mode = m;
	}

	function addMap(name) {
		const n = (name || '').trim();
		if (!n) return;
		cycle = [...cycle, n];
	}
	function addManual() {
		addMap(manualMap);
		manualMap = '';
	}
	function removeAt(i) {
		cycle = cycle.filter((_, idx) => idx !== i);
	}
	function moveUp(i) {
		if (i <= 0) return;
		const next = cycle.slice();
		[next[i - 1], next[i]] = [next[i], next[i - 1]];
		cycle = next;
	}
	function moveDown(i) {
		if (i >= cycle.length - 1) return;
		const next = cycle.slice();
		[next[i + 1], next[i]] = [next[i], next[i + 1]];
		cycle = next;
	}
	function duplicateEntry(i) {
		const next = cycle.slice();
		next.splice(i + 1, 0, cycle[i]);
		cycle = next;
	}

	function onDragStart(i, ev) {
		dragFrom = i;
		try { ev.dataTransfer.effectAllowed = 'move'; ev.dataTransfer.setData('text/plain', String(i)); } catch (_) {}
	}
	function onDragOver(i, ev) {
		ev.preventDefault();
		dragOver = i;
	}
	function onDrop(i, ev) {
		ev.preventDefault();
		const from = dragFrom;
		dragFrom = -1; dragOver = -1;
		if (from < 0 || from === i) return;
		const next = cycle.slice();
		const [item] = next.splice(from, 1);
		next.splice(i, 0, item);
		cycle = next;
	}
	function onDragEnd() { dragFrom = -1; dragOver = -1; }

	function revert() {
		cycle = original.slice();
		rawText = cycle.join('\n');
	}

	async function save() {
		// Make sure whatever mode the user edited in is reflected.
		if (mode === 'raw') syncFromRaw();
		saving = true; msg = '';
		try {
			const r = await api.serverSetMapcycle(serverId, cycle);
			msg = (r?.data ?? r?.Ok)?.message || 'Saved';
			await load();
		} catch (e) { msg = e.message; }
		finally { saving = false; }
	}
</script>

<div class="flex items-center justify-between mb-3">
	<h2 class="text-xl font-semibold">Mapcycle</h2>
	<div class="inline-flex rounded-lg border border-surface-800 bg-surface-900/50 p-0.5">
		<button
			class="px-3 py-1.5 text-xs rounded-md flex items-center gap-1.5 transition-colors {mode === 'visual' ? 'bg-surface-800 text-surface-100' : 'text-surface-400 hover:text-surface-200'}"
			onclick={() => switchMode('visual')}
		>
			<List size={14} /> Visual
		</button>
		<button
			class="px-3 py-1.5 text-xs rounded-md flex items-center gap-1.5 transition-colors {mode === 'raw' ? 'bg-surface-800 text-surface-100' : 'text-surface-400 hover:text-surface-200'}"
			onclick={() => switchMode('raw')}
		>
			<Code size={14} /> Raw text
		</button>
	</div>
</div>

<p class="text-sm text-surface-500 mb-3">
	File: <code class="text-surface-300">{mapcyclePath || '—'}</code> ·
	{cycle.length} map{cycle.length === 1 ? '' : 's'}
	{#if dirty}<span class="ml-2 text-amber-400">• unsaved changes</span>{/if}
</p>
{#if error}<div class="text-red-400 mb-2">{error}</div>{/if}
{#if msg}<div class="text-accent mb-2">{msg}</div>{/if}

{#if mode === 'visual'}
	<div class="grid grid-cols-1 lg:grid-cols-3 gap-4">
		<!-- Left / main: the ordered cycle -->
		<div class="lg:col-span-2 card p-0 overflow-hidden">
			<div class="px-4 py-3 border-b border-surface-800 text-xs font-medium uppercase tracking-wider text-surface-500 flex items-center justify-between">
				<span>Rotation order</span>
				<span class="text-surface-600 normal-case tracking-normal">Drag to reorder</span>
			</div>
			{#if loading}
				<div class="p-6 text-center text-surface-500 text-sm">Loading…</div>
			{:else if cycle.length === 0}
				<div class="p-6 text-center text-surface-500 text-sm">Mapcycle is empty. Add a map from the right.</div>
			{:else}
				<ol class="divide-y divide-surface-800/50">
					{#each cycle as m, i (i + ':' + m)}
						<li
							class="flex items-center gap-2 px-3 py-2 transition-colors
								{dragOver === i && dragFrom !== -1 && dragFrom !== i ? 'bg-accent/10 ring-1 ring-accent/40' : 'hover:bg-surface-800/30'}
								{dragFrom === i ? 'opacity-50' : ''}"
							draggable="true"
							ondragstart={(ev) => onDragStart(i, ev)}
							ondragover={(ev) => onDragOver(i, ev)}
							ondrop={(ev) => onDrop(i, ev)}
							ondragend={onDragEnd}
						>
							<span class="cursor-grab text-surface-600 hover:text-surface-300" title="Drag to reorder">
								<GripVertical size={16} />
							</span>
							<span class="w-8 text-right text-xs font-mono text-surface-500">{i + 1}</span>
							<span class="flex-1 font-mono text-sm text-surface-100">{m}</span>
							<div class="flex items-center gap-0.5">
								<button class="btn-ghost btn-sm" title="Move up" disabled={i === 0} onclick={() => moveUp(i)}>
									<ArrowUp size={14} />
								</button>
								<button class="btn-ghost btn-sm" title="Move down" disabled={i === cycle.length - 1} onclick={() => moveDown(i)}>
									<ArrowDown size={14} />
								</button>
								<button class="btn-ghost btn-sm" title="Duplicate" onclick={() => duplicateEntry(i)}>
									<Plus size={14} />
								</button>
								<button class="btn-ghost btn-sm text-red-400 hover:text-red-300" title="Remove" onclick={() => removeAt(i)}>
									<X size={14} />
								</button>
							</div>
						</li>
					{/each}
				</ol>
			{/if}
		</div>

		<!-- Right: add a map -->
		<div class="card p-4 h-fit">
			<h3 class="text-sm font-semibold text-surface-200 mb-3">Add a map</h3>

			<label class="block text-xs text-surface-500 mb-1" for="mc-filter">Search server maps</label>
			<input
				id="mc-filter"
				type="text"
				class="input mb-3"
				placeholder={availableLoading ? 'Loading…' : 'e.g. ut4_turnpike'}
				bind:value={addFilter}
			/>

			{#if availableMaps.length === 0 && !availableLoading}
				<p class="text-xs text-surface-500 mb-3">Server map list unavailable. Use manual entry below.</p>
			{:else}
				<div class="max-h-80 overflow-y-auto rounded border border-surface-800 bg-surface-950/50 mb-3">
					{#each suggestions as m}
						<button
							type="button"
							class="w-full flex items-center justify-between px-3 py-1.5 text-left text-sm font-mono text-surface-200 hover:bg-surface-800/60 transition-colors"
							onclick={() => addMap(m)}
						>
							<span>{m}</span>
							<Plus size={14} class="text-surface-500" />
						</button>
					{:else}
						<div class="px-3 py-4 text-center text-xs text-surface-500">
							{addFilter ? 'No maps match.' : 'All server maps already in cycle.'}
						</div>
					{/each}
				</div>
			{/if}

			<label class="block text-xs text-surface-500 mb-1" for="mc-manual">Or add manually</label>
			<div class="flex gap-2">
				<input
					id="mc-manual"
					type="text"
					class="input"
					placeholder="ut4_mapname"
					bind:value={manualMap}
					onkeydown={(e) => { if (e.key === 'Enter') addManual(); }}
				/>
				<button class="btn btn-secondary" onclick={addManual} disabled={!manualMap.trim()}>
					<Plus size={14} /> Add
				</button>
			</div>
			<p class="text-xs text-surface-600 mt-2">Tip: duplicates are allowed (same map can appear twice).</p>
		</div>
	</div>
{:else}
	<p class="text-xs text-surface-500 mb-2">One map per line. Blank lines are ignored.</p>
	<textarea class="input font-mono" rows="20" bind:value={rawText} disabled={loading}></textarea>
{/if}

<div class="mt-4 flex items-center gap-2">
	<button class="btn btn-primary" onclick={save} disabled={saving || loading || !dirty}>
		<Save size={14} /> {saving ? 'Saving…' : 'Save Mapcycle'}
	</button>
	<button class="btn btn-secondary" onclick={revert} disabled={saving || loading || !dirty}>
		<RotateCcw size={14} /> Revert
	</button>
	<button class="btn btn-secondary" onclick={load} disabled={saving || loading}>
		Reload from server
	</button>
</div>
