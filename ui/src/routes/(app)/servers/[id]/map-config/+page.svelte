<script>
	import { api } from '$lib/api.svelte.js';
	import { page } from '$app/stores';
	import MapConfigEditor from '$lib/mapconfig/MapConfigEditor.svelte';
	import { ArrowRightCircle, Play, RotateCcw, Save, Search } from 'lucide-svelte';

	let serverId = $derived(Number($page.params.id));

	let maps = $state([]);
	let currentMap = $state('');
	let configs = $state([]);
	let selectedMap = $state('');
	let editing = $state(null);
	let original = $state(null);
	let loading = $state(false);
	let saving = $state(false);
	let applying = $state(false);
	let resetting = $state(false);
	let switching = $state(false);
	let search = $state('');
	let error = $state('');
	let msg = $state('');

	let filteredMaps = $derived(
		search.trim()
			? maps.filter((m) => m.toLowerCase().includes(search.trim().toLowerCase()))
			: maps,
	);

	let configBySource = $derived(() => {
		const m = new Map();
		for (const c of configs) m.set(c.map_name, c);
		return m;
	});

	let dirty = $derived(
		editing && original ? JSON.stringify(editing) !== JSON.stringify(original) : false,
	);

	async function loadAll() {
		loading = true; error = '';
		try {
			// Pull maps from three independent sources and take the union so
			// the list is useful even when the server_maps scan hasn't run.
			const [scan, cfgs, cycle] = await Promise.all([
				api.serverMaps(serverId).catch(() => null),
				api.serverListMapConfigs(serverId),
				api.serverGetMapcycle(serverId).catch(() => null),
			]);
			let scanList = Array.isArray(scan?.maps) ? scan.maps : [];
			// Auto-kick a scan if the cache is empty — the scheduled background
			// scan may not have run yet.
			if (!scanList.length) {
				try {
					await api.serverRefreshMaps(serverId);
					const scan2 = await api.serverMaps(serverId);
					scanList = Array.isArray(scan2?.maps) ? scan2.maps : [];
				} catch (_) { /* non-fatal */ }
			}
			const scanNames = scanList
				.map((x) => (typeof x === 'string' ? x : x?.map_name))
				.filter(Boolean);
			const cycleNames = Array.isArray(cycle?.maps)
				? cycle.maps.map((x) => (typeof x === 'string' ? x : x?.name || x?.map_name)).filter(Boolean)
				: [];
			const list2 = cfgs?.configs || cfgs?.data?.configs || cfgs?.Ok?.data?.configs || [];
			configs = Array.isArray(list2) ? list2 : [];
			const cfgNames = configs.map((c) => c.map_name).filter(Boolean);
			const set = new Set([...scanNames, ...cycleNames, ...cfgNames]);
			maps = Array.from(set).sort();
			try {
				const s = await api.server(serverId);
				currentMap = s?.server?.current_map || s?.current_map || '';
			} catch { /* ignore */ }
			if (currentMap && !maps.includes(currentMap)) {
				maps = [...maps, currentMap].sort();
			}
			if (!selectedMap && maps.length) {
				await selectMap(currentMap && maps.includes(currentMap) ? currentMap : maps[0]);
			}
		} catch (e) { error = e.message || String(e); }
		finally { loading = false; }
	}

	// ClientResponse is serialized as { response_type: "Ok", data: { message, data: {...} } }
	// so the actual payload lives at r.data.data.
	function extractConfig(r) {
		return (
			r?.data?.data?.config ||
			r?.data?.config ||
			r?.Ok?.data?.config ||
			r?.config ||
			null
		);
	}

	async function selectMap(name) {
		if (!name) return;
		msg = ''; error = '';
		selectedMap = name;
		editing = null; original = null;
		try {
			const r = await api.serverEnsureMapConfig(serverId, name);
			const cfg = extractConfig(r);
			if (cfg) {
				editing = structuredClone(cfg);
				original = structuredClone(cfg);
				const idx = configs.findIndex((c) => c.map_name === cfg.map_name);
				if (idx >= 0) configs[idx] = cfg;
				else configs = [...configs, cfg];
			} else {
				error = 'Could not load config for ' + name;
			}
		} catch (e) { error = e.message || String(e); }
	}

	async function save() {
		if (!editing) return;
		saving = true; msg = ''; error = '';
		try {
			editing.source = 'user';
			await api.serverSaveMapConfig(serverId, editing);
			original = structuredClone(editing);
			const idx = configs.findIndex((c) => c.map_name === editing.map_name);
			if (idx >= 0) configs[idx] = structuredClone(editing);
			msg = 'Saved.';
		} catch (e) { error = e.message || String(e); }
		finally { saving = false; }
	}

	function discard() {
		if (!original) return;
		editing = structuredClone(original);
	}

	async function switchTo() {
		if (!selectedMap) return;
		switching = true; msg = ''; error = '';
		try {
			await api.serverChangeMap(serverId, selectedMap);
			msg = `Map change to ${selectedMap} requested.`;
		} catch (e) { error = e.message || String(e); }
		finally { switching = false; }
	}

	async function applyNow() {
		if (!selectedMap) return;
		applying = true; msg = ''; error = '';
		try {
			await api.serverApplyMapConfig(serverId, selectedMap);
			msg = 'Applied config to live server.';
		} catch (e) { error = e.message || String(e); }
		finally { applying = false; }
	}

	async function resetToDefault() {
		if (!selectedMap) return;
		if (!confirm(`Reset config for ${selectedMap} to its default?`)) return;
		resetting = true; msg = ''; error = '';
		try {
			const r = await api.serverResetMapConfig(serverId, selectedMap);
			const cfg = extractConfig(r);
			if (cfg) {
				editing = structuredClone(cfg);
				original = structuredClone(cfg);
				const idx = configs.findIndex((c) => c.map_name === cfg.map_name);
				if (idx >= 0) configs[idx] = cfg;
			}
			msg = 'Reset to defaults.';
		} catch (e) { error = e.message || String(e); }
		finally { resetting = false; }
	}

	$effect(() => { if (serverId) loadAll(); });
</script>

<div class="flex h-full min-h-[70vh] gap-4">
	<aside class="flex w-72 shrink-0 flex-col rounded-lg border border-zinc-800 bg-zinc-900/40">
		<div class="border-b border-zinc-800 p-3">
			<div class="relative">
				<Search class="pointer-events-none absolute left-2 top-2.5 h-3.5 w-3.5 text-zinc-500" />
				<input
					type="text"
					class="w-full rounded border border-zinc-700 bg-zinc-950 py-1.5 pl-7 pr-2 text-sm"
					placeholder="Search maps…"
					bind:value={search}
				/>
			</div>
			<div class="mt-2 text-xs text-zinc-500">
				{filteredMaps.length} / {maps.length} maps
			</div>
		</div>
		<div class="flex-1 overflow-y-auto">
			{#if loading && !maps.length}
				<div class="p-3 text-xs text-zinc-500">Loading…</div>
			{:else if !filteredMaps.length}
				<div class="p-3 text-xs text-zinc-500">No maps.</div>
			{:else}
				{#each filteredMaps as m}
					{@const cfg = configBySource().get(m)}
					{@const src = cfg?.source || 'auto'}
					<button
						type="button"
						class="flex w-full items-center justify-between gap-2 border-l-2 px-3 py-2 text-left text-sm transition {selectedMap === m
							? 'border-blue-500 bg-blue-500/10 text-blue-200'
							: 'border-transparent text-zinc-300 hover:bg-zinc-800/50'}"
						onclick={() => selectMap(m)}
					>
						<span class="truncate">{m}</span>
						<span class="flex shrink-0 items-center gap-1">
							{#if m === currentMap}
								<span class="rounded bg-emerald-500/20 px-1.5 py-0.5 text-[10px] font-medium text-emerald-300">live</span>
							{/if}
							{#if src === 'user'}
								<span class="rounded bg-blue-500/20 px-1.5 py-0.5 text-[10px] font-medium text-blue-300">edited</span>
							{:else if src === 'default_seed'}
								<span class="rounded bg-zinc-700/50 px-1.5 py-0.5 text-[10px] font-medium text-zinc-400">default</span>
							{:else}
								<span class="rounded bg-amber-500/15 px-1.5 py-0.5 text-[10px] font-medium text-amber-300">auto</span>
							{/if}
						</span>
					</button>
				{/each}
			{/if}
		</div>
	</aside>

	<section class="flex min-w-0 flex-1 flex-col">
		{#if !editing}
			<div class="flex flex-1 items-center justify-center text-sm text-zinc-500">
				{loading ? 'Loading…' : 'Select a map on the left to edit its config.'}
			</div>
		{:else}
			<div class="mb-3 flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-900/40 p-3">
				<div class="min-w-0 flex-1">
					<div class="truncate text-base font-semibold text-zinc-100">{editing.map_name}</div>
					<div class="text-xs text-zinc-500">
						source: <span class="font-mono">{editing.source || 'auto'}</span>
						{#if dirty}
							<span class="ml-2 rounded bg-amber-500/15 px-1.5 py-0.5 text-[10px] text-amber-300">unsaved changes</span>
						{/if}
					</div>
				</div>
				<button class="btn-secondary flex items-center gap-1 text-sm" onclick={switchTo} disabled={switching}>
					<ArrowRightCircle class="h-4 w-4" />
					{switching ? '…' : 'Switch to map'}
				</button>
				<button class="btn-secondary flex items-center gap-1 text-sm" onclick={applyNow} disabled={applying || dirty}>
					<Play class="h-4 w-4" />
					{applying ? '…' : 'Apply now'}
				</button>
				<button class="btn-secondary flex items-center gap-1 text-sm" onclick={resetToDefault} disabled={resetting}>
					<RotateCcw class="h-4 w-4" />
					{resetting ? '…' : 'Reset to default'}
				</button>
			</div>

			{#if error}<div class="mb-2 rounded border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-300">{error}</div>{/if}
			{#if msg}<div class="mb-2 rounded border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-sm text-emerald-300">{msg}</div>{/if}

			<div class="flex-1 overflow-y-auto pr-1">
				<MapConfigEditor bind:config={editing} disabled={saving || resetting} />
			</div>

			<div class="mt-3 flex items-center justify-end gap-2 border-t border-zinc-800 pt-3">
				<button class="btn-secondary text-sm" onclick={discard} disabled={!dirty || saving}>
					Discard
				</button>
				<button class="btn flex items-center gap-1 text-sm" onclick={save} disabled={!dirty || saving}>
					<Save class="h-4 w-4" />
					{saving ? 'Saving…' : 'Save'}
				</button>
			</div>
		{/if}
	</section>
</div>
