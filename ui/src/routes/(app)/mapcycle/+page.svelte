<script>
	import { onMount } from 'svelte';
	import { api } from '$lib/api.svelte.js';
	import { GripVertical, Plus, Trash2, Save, RotateCcw, Globe, RefreshCw, Clock, AlertTriangle } from 'lucide-svelte';
	import MapRepoBrowser from '$lib/components/MapRepoBrowser.svelte';
	import MissingMapsDialog from '$lib/components/MissingMapsDialog.svelte';
	import MapConfigCreateDialog from '$lib/components/MapConfigCreateDialog.svelte';

	let maps = $state([]);
	/** Full entries from the server_maps cache: { map_name, pending_restart, pk3_filename, ... } */
	let availableMaps = $state([]);
	let originalMaps = $state([]);
	let lastScanAt = $state(null);
	let lastScanOk = $state(true);
	let lastScanError = $state(null);
	let refreshing = $state(false);
	let newMap = $state('');
	let loading = $state(true);
	let saving = $state(false);
	let message = $state('');
	let error = $state('');
	let dragIndex = $state(null);
	let showRepo = $state(false);
	let showMissing = $state(false);
	let missingList = $state([]);
	let showMcCreate = $state(false);
	let mcCreateFile = $state('');

	function promptMapConfig(filename) {
		try {
			if (sessionStorage.getItem('r3.skipMapConfigPrompt') === '1') return;
		} catch (_) {}
		mcCreateFile = filename;
		showMcCreate = true;
	}

	function applyMapListResponse(d) {
		availableMaps = (d?.maps || []).map((m) =>
			typeof m === 'string'
				? { map_name: m, pending_restart: false, pk3_filename: null }
				: m
		);
		lastScanAt = d?.last_scan_at || null;
		lastScanOk = d?.last_scan_ok !== false;
		lastScanError = d?.last_scan_error || null;
	}

	async function reloadAvailable() {
		try { applyMapListResponse(await api.mapList()); } catch (_) {}
	}

	async function refreshScan() {
		refreshing = true;
		error = '';
		try {
			await api.refreshMaps();
			await reloadAvailable();
			message = 'Map list refreshed from server.';
		} catch (e) {
			error = 'Refresh failed: ' + e.message;
		}
		refreshing = false;
	}

	function formatLastScan(ts) {
		if (!ts) return 'never';
		const then = new Date(ts).getTime();
		const diff = Math.max(0, Date.now() - then);
		const mins = Math.floor(diff / 60000);
		if (mins < 1) return 'just now';
		if (mins < 60) return `${mins}m ago`;
		const hrs = Math.floor(mins / 60);
		if (hrs < 24) return `${hrs}h ago`;
		return `${Math.floor(hrs / 24)}d ago`;
	}

	onMount(async () => {
		try {
			const [cycleData, mapListData] = await Promise.all([
				api.mapcycle(),
				api.mapList()
			]);
			maps = cycleData.maps || [];
			originalMaps = [...maps];
			applyMapListResponse(mapListData);
		} catch (e) {
			error = 'Failed to load mapcycle: ' + e.message;
		}
		loading = false;
	});

	function addMap() {
		const name = newMap.trim();
		if (name && !maps.includes(name)) {
			maps = [...maps, name];
			newMap = '';
		}
	}

	function removeMap(index) {
		maps = maps.filter((_, i) => i !== index);
	}

	function moveMap(from, to) {
		if (to < 0 || to >= maps.length) return;
		const updated = [...maps];
		const [item] = updated.splice(from, 1);
		updated.splice(to, 0, item);
		maps = updated;
	}

	function handleDragStart(index) {
		dragIndex = index;
	}

	function handleDragOver(e, index) {
		e.preventDefault();
		if (dragIndex !== null && dragIndex !== index) {
			moveMap(dragIndex, index);
			dragIndex = index;
		}
	}

	function handleDragEnd() {
		dragIndex = null;
	}

	async function save() {
		saving = true;
		message = '';
		error = '';
		try {
			await api.updateMapcycle(maps);
			originalMaps = [...maps];
			message = 'Mapcycle saved successfully.';
		} catch (e) {
			error = 'Failed to save: ' + e.message;
		}
		saving = false;
	}

	async function trySave() {
		try {
			const r = await api.localMissingMaps(maps);
			const list = r?.missing || [];
			if (list.length > 0) {
				missingList = list;
				showMissing = true;
				return;
			}
		} catch (_) { /* non-fatal */ }
		await save();
	}

	function reset() {
		maps = [...originalMaps];
		message = '';
		error = '';
	}

	$effect(() => {
		if (message || error) {
			const timer = setTimeout(() => { message = ''; error = ''; }, 5000);
			return () => clearTimeout(timer);
		}
	});

	let hasChanges = $derived(JSON.stringify(maps) !== JSON.stringify(originalMaps));

	let suggestions = $derived(
		newMap.length > 0
			? availableMaps
				.filter((m) =>
					m.map_name.toLowerCase().includes(newMap.toLowerCase()) &&
					!maps.includes(m.map_name)
				)
				.slice(0, 8)
			: []
	);

	let pendingCount = $derived(availableMaps.filter((m) => m.pending_restart).length);
</script>

<svelte:head><title>Mapcycle Editor | R3</title></svelte:head>

<div class="p-6 max-w-3xl mx-auto">
	<div class="flex items-center justify-between mb-6">
		<h1 class="text-2xl font-bold text-white">Mapcycle Editor</h1>
		<div class="flex gap-2">
			<button onclick={() => (showRepo = true)}
				class="flex items-center gap-1.5 px-3 py-2 bg-zinc-700 text-zinc-200 rounded-lg hover:bg-zinc-600 text-sm">
				<Globe size={14}/> Browse repo
			</button>
			<button onclick={reset} disabled={!hasChanges || saving}
				class="flex items-center gap-1.5 px-3 py-2 bg-zinc-700 text-zinc-300 rounded-lg hover:bg-zinc-600 disabled:opacity-40 text-sm">
				<RotateCcw size={14}/> Reset
			</button>
			<button onclick={trySave} disabled={!hasChanges || saving}
				class="flex items-center gap-1.5 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-500 disabled:opacity-40 text-sm font-medium">
				<Save size={14}/> {saving ? 'Saving...' : 'Save'}
			</button>
		</div>
	</div>

	{#if message}
		<div class="mb-4 p-3 bg-green-500/20 border border-green-500/40 rounded-lg text-green-300 text-sm">{message}</div>
	{/if}
	{#if error}
		<div class="mb-4 p-3 bg-red-500/20 border border-red-500/40 rounded-lg text-red-300 text-sm">{error}</div>
	{/if}

	{#if loading}
		<div class="text-zinc-400 text-center py-12">Loading mapcycle...</div>
	{:else}
		<!-- Installed-map cache status strip -->
		<div class="mb-4 flex items-center justify-between gap-3 px-3 py-2 bg-zinc-800/40 border border-zinc-700/50 rounded-lg text-xs">
			<div class="flex items-center gap-2 text-zinc-400">
				<Clock size={12}/>
				<span>
					{availableMaps.length} installed map{availableMaps.length !== 1 ? 's' : ''}
					· last scanned {formatLastScan(lastScanAt)}
					{#if !lastScanOk && lastScanError}
						<span class="text-red-400">· {lastScanError}</span>
					{/if}
					{#if pendingCount > 0}
						<span class="text-amber-300">· {pendingCount} pending restart</span>
					{/if}
				</span>
			</div>
			<button onclick={refreshScan} disabled={refreshing}
				class="flex items-center gap-1.5 px-2 py-1 bg-zinc-700 text-zinc-200 rounded hover:bg-zinc-600 disabled:opacity-40 text-xs">
				<RefreshCw size={12} class={refreshing ? 'animate-spin' : ''}/>
				{refreshing ? 'Refreshing…' : 'Refresh'}
			</button>
		</div>

		<!-- Add map -->
		<div class="mb-6 relative">
			<div class="flex gap-2">
				<input type="text" bind:value={newMap} placeholder="Add map name..."
					onkeydown={(e) => { if (e.key === 'Enter') addMap(); }}
					class="flex-1 px-3 py-2 bg-zinc-800 border border-zinc-700 rounded-lg text-white placeholder-zinc-500 focus:outline-none focus:border-blue-500 text-sm"/>
				<button onclick={addMap} class="flex items-center gap-1.5 px-4 py-2 bg-zinc-700 text-white rounded-lg hover:bg-zinc-600 text-sm">
					<Plus size={14}/> Add
				</button>
			</div>
			{#if suggestions.length > 0}
				<div class="absolute top-full left-0 right-12 mt-1 bg-zinc-800 border border-zinc-700 rounded-lg shadow-xl z-10 max-h-48 overflow-y-auto">
					{#each suggestions as s}
						<button onclick={() => { newMap = s.map_name; addMap(); }}
							class="w-full text-left px-3 py-2 text-sm text-zinc-300 hover:bg-zinc-700 hover:text-white flex items-center justify-between gap-2">
							<span>{s.map_name}</span>
							{#if s.pending_restart}
								<span class="inline-flex items-center gap-1 text-[10px] uppercase tracking-wide text-amber-300 bg-amber-500/10 border border-amber-500/30 rounded px-1.5 py-0.5">
									<AlertTriangle size={10}/> pending restart
								</span>
							{/if}
						</button>
					{/each}
				</div>
			{/if}
		</div>

		<!-- Map list -->
		<div class="space-y-1">
			{#each maps as map, i}
				<div draggable="true"
					ondragstart={() => handleDragStart(i)}
					ondragover={(e) => handleDragOver(e, i)}
					ondragend={handleDragEnd}
					class="flex items-center gap-3 px-3 py-2.5 bg-zinc-800/60 border border-zinc-700/50 rounded-lg group hover:border-zinc-600 transition-colors {dragIndex === i ? 'opacity-50' : ''}">
					<span class="text-zinc-500 cursor-grab active:cursor-grabbing"><GripVertical size={16}/></span>
					<span class="text-zinc-500 text-xs font-mono w-6 text-right">{i + 1}</span>
					<span class="flex-1 text-white text-sm font-medium">{map}</span>
					{#if availableMaps.find((a) => a.map_name === map)?.pending_restart}
						<span class="inline-flex items-center gap-1 text-[10px] uppercase tracking-wide text-amber-300 bg-amber-500/10 border border-amber-500/30 rounded px-1.5 py-0.5">
							<AlertTriangle size={10}/> pending restart
						</span>
					{/if}
					<div class="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
						<button onclick={() => moveMap(i, i - 1)} disabled={i === 0}
							class="p-1 text-zinc-400 hover:text-white disabled:opacity-20 text-xs">▲</button>
						<button onclick={() => moveMap(i, i + 1)} disabled={i === maps.length - 1}
							class="p-1 text-zinc-400 hover:text-white disabled:opacity-20 text-xs">▼</button>
						<button onclick={() => removeMap(i)}
							class="p-1 text-zinc-400 hover:text-red-400"><Trash2 size={14}/></button>
					</div>
				</div>
			{/each}
		</div>

		{#if maps.length === 0}
			<div class="text-center py-12 text-zinc-500">No maps in cycle. Add maps above.</div>
		{/if}

		<div class="mt-4 text-xs text-zinc-500">{maps.length} map{maps.length !== 1 ? 's' : ''} in rotation</div>
	{/if}
</div>

<MapRepoBrowser
	bind:open={showRepo}
	serverId={null}
	onimported={(fn) => {
		const stem = fn.replace(/\.pk3$/i, '');
		if (!maps.includes(stem)) maps = [...maps, stem];
		reloadAvailable();
		promptMapConfig(fn);
	}} />

<MissingMapsDialog
	bind:open={showMissing}
	serverId={null}
	missing={missingList}
	onproceed={save}
	onimported={(fn) => { reloadAvailable(); promptMapConfig(fn); }} />

<MapConfigCreateDialog bind:open={showMcCreate} filename={mcCreateFile} />
