<script>
	import { api } from '$lib/api.svelte.js';
	import { page } from '$app/stores';
	import {
		Save, RefreshCw, Search, ChevronDown, ChevronRight,
		Info, RotateCcw, Plus, Trash2, FileText, LayoutGrid, AlertTriangle,
	} from 'lucide-svelte';
	import { CVARS, SECTIONS, GEAR_ITEMS, getCvar } from '$lib/urt-cvars.js';
	import { parseCfg, serializeCfg } from '$lib/urt-cfg-parse.js';

	let serverId = $derived(Number($page.params.id));

	let cfgPath = $state('');
	let originalText = $state('');
	let loading = $state(false);
	let saving = $state(false);
	let error = $state('');
	let msg = $state('');

	let mode = $state('form');
	/** @type {Record<string,string>} */
	let values = $state({});
	let unknownKeys = $state([]);
	let rawText = $state('');
	let parsed = $state(null);
	let search = $state('');
	let showAdvanced = $state(false);
	/** @type {Record<string,boolean>} */
	let sectionOpen = $state({});
	/** @type {Array<{key:string,value:string}>} */
	let newCvars = $state([]);
	let removedKeys = $state(new Set());

	let gearCvarKey = $state(null);
	let gearSelection = $state({});

	async function load() {
		loading = true; error = ''; msg = '';
		try {
			const r = await api.serverGetServerCfg(serverId);
			if (r?.response_type === 'Error' || r?.Error) {
				error = r.message || r.Error?.message || 'Failed to load server.cfg';
				return;
			}
			const d = r?.data ?? r?.ServerCfg ?? {};
			cfgPath = d.path || '';
			applyRawText(d.contents || '');
			loadSectionState();
		} catch (e) {
			error = e.message;
		} finally {
			loading = false;
		}
	}

	function applyRawText(text) {
		originalText = text;
		rawText = text;
		parsed = parseCfg(text);
		const newValues = {};
		const unknowns = [];
		for (const [k, v] of Object.entries(parsed.cvars)) {
			newValues[k] = v.value;
			if (!getCvar(k)) unknowns.push(k);
		}
		values = newValues;
		unknownKeys = unknowns.sort();
		newCvars = [];
		removedKeys = new Set();
	}

	$effect(() => { load(); });

	function loadSectionState() {
		try {
			const raw = localStorage.getItem(`r3.cfgEditor.sectionOpen.${serverId}`);
			const saved = raw ? JSON.parse(raw) : {};
			const out = {};
			for (const s of SECTIONS) out[s.id] = saved[s.id] ?? s.defaultOpen;
			out.__advanced__ = saved.__advanced__ ?? false;
			sectionOpen = out;
		} catch {
			const out = {};
			for (const s of SECTIONS) out[s.id] = s.defaultOpen;
			sectionOpen = out;
		}
	}
	function toggleSection(id) {
		sectionOpen = { ...sectionOpen, [id]: !sectionOpen[id] };
		try {
			localStorage.setItem(`r3.cfgEditor.sectionOpen.${serverId}`, JSON.stringify(sectionOpen));
		} catch {}
	}

	let dirty = $derived.by(() => {
		if (!parsed) return false;
		if (removedKeys.size > 0) return true;
		if (newCvars.some((c) => c.key.trim())) return true;
		for (const [k, v] of Object.entries(values)) {
			const orig = parsed.cvars[k]?.value;
			if (orig !== undefined && String(orig) !== String(v)) return true;
		}
		return false;
	});

	function switchToRaw() {
		if (mode === 'raw') return;
		rawText = buildSerialized();
		mode = 'raw';
	}
	function switchToForm() {
		if (mode === 'form') return;
		applyRawText(rawText);
		mode = 'form';
	}

	function buildSerialized() {
		if (!parsed) return originalText;
		const desired = { ...values };
		for (const k of removedKeys) delete desired[k];
		const valid = newCvars
			.filter((c) => c.key.trim())
			.map((c) => ({ key: c.key.trim(), value: c.value ?? '' }));
		if (removedKeys.size === 0) {
			return serializeCfg(parsed, desired, valid);
		}
		const filtered = {
			original: parsed.original,
			cvars: { ...parsed.cvars },
			lines: parsed.lines.filter((ln) => !(ln.kind === 'set' && removedKeys.has(ln.key))),
		};
		for (const k of removedKeys) delete filtered.cvars[k];
		return serializeCfg(filtered, desired, valid);
	}

	async function save() {
		saving = true; msg = ''; error = '';
		try {
			const toSave = mode === 'raw' ? rawText : buildSerialized();
			const r = await api.serverSaveServerCfg(serverId, cfgPath, toSave);
			if (r?.response_type === 'Error' || r?.Error) {
				error = r.message || r.Error?.message || 'Save failed';
				return;
			}
			msg = (r?.data ?? r?.Ok)?.message || 'Saved';
			applyRawText(toSave);
		} catch (e) {
			error = e.message;
		} finally {
			saving = false;
		}
	}

	async function reload() {
		if (dirty && !confirm('Discard unsaved changes and reload from disk?')) return;
		await load();
	}

	function displayValue(cvar) { return values[cvar.key] ?? String(cvar.default ?? ''); }
	function isOverridden(cvar) {
		const cur = values[cvar.key];
		if (cur === undefined) return false;
		if (removedKeys.has(cvar.key)) return false;
		return String(cur) !== String(cvar.default ?? '');
	}
	function setValue(key, val) {
		values = { ...values, [key]: val };
		if (removedKeys.has(key)) {
			const ns = new Set(removedKeys);
			ns.delete(key);
			removedKeys = ns;
		}
	}
	function resetToDefault(cvar) {
		if (parsed?.cvars[cvar.key]) removedKeys = new Set(removedKeys).add(cvar.key);
		const { [cvar.key]: _, ...rest } = values;
		values = rest;
	}
	function parseBitmask(val, flags) {
		const n = Number(val) || 0;
		const set = {};
		for (const f of flags) set[f.value] = (n & f.value) !== 0;
		return set;
	}

	function matchesSearch(cvar) {
		if (!search.trim()) return true;
		const q = search.toLowerCase();
		return (
			cvar.key.toLowerCase().includes(q) ||
			cvar.label.toLowerCase().includes(q) ||
			(cvar.help || '').toLowerCase().includes(q)
		);
	}
	let filteredSections = $derived.by(() => {
		return SECTIONS.map((s) => {
			const rows = CVARS
				.filter((c) => c.section === s.id)
				.filter((c) => showAdvanced || !c.advanced)
				.filter(matchesSearch);
			const overrides = rows.filter(isOverridden).length;
			return { ...s, rows, overrides };
		});
	});

	let unknownRows = $derived.by(() => {
		if (!search.trim()) return unknownKeys;
		const q = search.toLowerCase();
		return unknownKeys.filter(
			(k) => k.toLowerCase().includes(q) || (values[k] || '').toLowerCase().includes(q),
		);
	});

	function openGearPicker(key) {
		gearCvarKey = key;
		const cur = values[key] ?? '';
		// g_gear in UrT is a BAN list. Invert in UI so checked = allowed.
		const sel = {};
		for (const item of GEAR_ITEMS) sel[item.code] = !cur.includes(item.code);
		gearSelection = sel;
	}
	function applyGearPicker() {
		// Persist unchecked codes (the banned items).
		const str = GEAR_ITEMS.filter((i) => !gearSelection[i.code]).map((i) => i.code).join('');
		setValue(gearCvarKey, str);
		gearCvarKey = null;
	}
	function closeGearPicker() { gearCvarKey = null; }

	function addCustomCvar() { newCvars = [...newCvars, { key: '', value: '' }]; }
	function updateNewCvar(i, field, val) {
		newCvars = newCvars.map((c, idx) => (idx === i ? { ...c, [field]: val } : c));
	}
	function removeNewCvar(i) { newCvars = newCvars.filter((_, idx) => idx !== i); }
	function removeUnknown(key) {
		removedKeys = new Set(removedKeys).add(key);
		const { [key]: _, ...rest } = values;
		values = rest;
		unknownKeys = unknownKeys.filter((k) => k !== key);
	}
</script>

<svelte:head><title>server.cfg Editor | R3</title></svelte:head>

<div class="sticky top-0 z-10 -mx-4 mb-4 border-b border-surface-800 bg-surface-950/90 px-4 py-3 backdrop-blur">
	<div class="flex flex-wrap items-center gap-3">
		<div class="min-w-0 flex-1">
			<h2 class="flex items-center gap-2 text-lg font-semibold text-surface-100">
				server.cfg Editor
				{#if dirty}<span class="badge-yellow">● unsaved</span>{/if}
			</h2>
			{#if cfgPath}
				<p class="truncate text-xs text-surface-500"><code class="text-surface-400">{cfgPath}</code></p>
			{/if}
		</div>

		<div class="flex overflow-hidden rounded-lg border border-surface-700 bg-surface-800/50">
			<button
				class="flex items-center gap-1.5 px-3 py-1.5 text-xs {mode === 'form' ? 'bg-accent text-white' : 'text-surface-400 hover:text-surface-200'}"
				onclick={switchToForm}
			>
				<LayoutGrid class="h-3.5 w-3.5" /> Form
			</button>
			<button
				class="flex items-center gap-1.5 px-3 py-1.5 text-xs {mode === 'raw' ? 'bg-accent text-white' : 'text-surface-400 hover:text-surface-200'}"
				onclick={switchToRaw}
			>
				<FileText class="h-3.5 w-3.5" /> Raw
			</button>
		</div>

		<button class="btn-secondary btn-sm" onclick={reload} disabled={loading || saving}>
			<RefreshCw class="h-3.5 w-3.5 {loading ? 'animate-spin' : ''}" /> Reload
		</button>
		<button class="btn-primary btn-sm" onclick={save} disabled={saving || loading || !cfgPath}>
			<Save class="h-3.5 w-3.5" /> {saving ? 'Saving…' : 'Save'}
		</button>
	</div>

	{#if mode === 'form'}
		<div class="mt-3 flex flex-wrap items-center gap-3">
			<div class="relative flex-1 min-w-[200px]">
				<Search class="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-surface-500" />
				<input
					type="text"
					bind:value={search}
					placeholder="Search cvars (key, label, or help)…"
					class="input pl-8 text-sm"
				/>
			</div>
			<label class="flex cursor-pointer items-center gap-2 text-xs text-surface-400">
				<input type="checkbox" bind:checked={showAdvanced} class="h-4 w-4 accent-accent" />
				Show advanced
			</label>
		</div>
	{/if}
</div>

{#if error}
	<div class="mb-3 flex items-start gap-2 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-300">
		<AlertTriangle class="mt-0.5 h-4 w-4 flex-shrink-0" />
		<span>{error}</span>
	</div>
{/if}
{#if msg}
	<div class="mb-3 rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-sm text-emerald-300">{msg}</div>
{/if}

{#if mode === 'raw'}
	<textarea class="input font-mono text-xs" rows="32" bind:value={rawText} disabled={loading}></textarea>
	<p class="mt-2 text-xs text-surface-500">Edit the file directly. Switching back to Form mode re-parses this content.</p>
{:else if loading}
	<div class="text-sm text-surface-500">Loading…</div>
{:else if !parsed}
	<div class="text-sm text-surface-500">No config loaded.</div>
{:else}
	<div class="space-y-3">
		{#each filteredSections as sec (sec.id)}
			<section class="card overflow-hidden">
				<button
					type="button"
					class="flex w-full items-center justify-between gap-3 px-4 py-3 text-left hover:bg-surface-800/30"
					onclick={() => toggleSection(sec.id)}
				>
					<div class="flex items-center gap-2">
						{#if sectionOpen[sec.id]}
							<ChevronDown class="h-4 w-4 text-surface-500" />
						{:else}
							<ChevronRight class="h-4 w-4 text-surface-500" />
						{/if}
						<h3 class="text-sm font-semibold text-surface-100">{sec.title}</h3>
						{#if sec.overrides > 0}<span class="badge-blue">{sec.overrides} customized</span>{/if}
					</div>
					<span class="text-xs text-surface-500">{sec.rows.length} settings</span>
				</button>

				{#if sectionOpen[sec.id]}
					<div class="divide-y divide-surface-800 border-t border-surface-800">
						{#each sec.rows as cvar (cvar.key)}
							{@const overridden = isOverridden(cvar)}
							<div class="grid grid-cols-1 gap-2 px-4 py-3 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] sm:items-start {overridden ? 'border-l-2 border-accent/60 bg-accent/5' : ''}">
								<div class="min-w-0">
									<div class="flex items-center gap-2">
										<label for="cfg-{cvar.key}" class="text-sm font-medium text-surface-200">{cvar.label}</label>
										{#if cvar.advanced}<span class="badge-gray text-[10px]">adv</span>{/if}
									</div>
									<div class="flex items-center gap-2">
										<code class="text-[11px] text-surface-500">{cvar.key}</code>
										{#if cvar.help}
											<span class="group relative inline-flex">
												<Info class="h-3 w-3 text-surface-600 hover:text-surface-400 cursor-help" />
												<span class="pointer-events-none absolute left-4 top-0 z-20 hidden w-64 rounded-lg border border-surface-700 bg-surface-800 px-2.5 py-1.5 text-[11px] text-surface-300 shadow-lg group-hover:block">
													{cvar.help}
												</span>
											</span>
										{/if}
									</div>
								</div>

								<div class="min-w-0">
									{#if cvar.type === 'bool01'}
										<select id="cfg-{cvar.key}" class="input text-sm" value={displayValue(cvar)} onchange={(e) => setValue(cvar.key, e.currentTarget.value)}>
											<option value="0">Off (0)</option>
											<option value="1">On (1)</option>
										</select>
									{:else if cvar.type === 'enum'}
										<select id="cfg-{cvar.key}" class="input text-sm" value={displayValue(cvar)} onchange={(e) => setValue(cvar.key, e.currentTarget.value)}>
											{#each cvar.options as opt (opt.value)}
												<option value={opt.value}>{opt.label}</option>
											{/each}
										</select>
									{:else if cvar.type === 'int' || cvar.type === 'float'}
										<input
											id="cfg-{cvar.key}"
											type="number"
											class="input text-sm"
											step={cvar.type === 'float' ? '0.01' : '1'}
											min={cvar.min ?? undefined}
											max={cvar.max ?? undefined}
											value={displayValue(cvar)}
											oninput={(e) => setValue(cvar.key, e.currentTarget.value)}
										/>
									{:else if cvar.type === 'gear'}
										<div class="flex gap-2">
											<input
												id="cfg-{cvar.key}"
												type="text"
												class="input flex-1 font-mono text-sm"
												placeholder="e.g. GAIKWNEMLOQURSTUVXZ"
												value={displayValue(cvar)}
												oninput={(e) => setValue(cvar.key, e.currentTarget.value)}
											/>
											<button type="button" class="btn-secondary btn-sm whitespace-nowrap" onclick={() => openGearPicker(cvar.key)}>Picker</button>
										</div>
									{:else if cvar.type === 'bitmask'}
										<div class="space-y-2">
											<input
												id="cfg-{cvar.key}"
												type="number"
												class="input text-sm"
												min="0"
												value={displayValue(cvar)}
												oninput={(e) => setValue(cvar.key, e.currentTarget.value)}
											/>
											<details class="rounded-lg border border-surface-800 bg-surface-900/60">
												<summary class="cursor-pointer px-3 py-1.5 text-xs text-surface-400 hover:text-surface-200">Flag picker</summary>
												<div class="grid grid-cols-2 gap-1 p-2 sm:grid-cols-3">
													{#each cvar.flags as fl (fl.value)}
														{@const bm = parseBitmask(displayValue(cvar), cvar.flags)}
														<label class="flex cursor-pointer items-center gap-1.5 rounded px-1.5 py-0.5 text-[11px] text-surface-300 hover:bg-surface-800">
															<input
																type="checkbox"
																class="h-3.5 w-3.5 accent-accent"
																checked={bm[fl.value]}
																onchange={(e) => {
																	const cur = Number(displayValue(cvar)) || 0;
																	const next = e.currentTarget.checked ? cur | fl.value : cur & ~fl.value;
																	setValue(cvar.key, String(next));
																}}
															/>
															<span>{fl.label}</span>
															<span class="text-surface-600">({fl.value})</span>
														</label>
													{/each}
												</div>
											</details>
										</div>
									{:else}
										<input
											id="cfg-{cvar.key}"
											type="text"
											class="input text-sm"
											value={displayValue(cvar)}
											oninput={(e) => setValue(cvar.key, e.currentTarget.value)}
										/>
									{/if}
								</div>

								<div class="flex items-center gap-2 sm:justify-end">
									{#if overridden}
										<button
											type="button"
											class="btn-ghost btn-sm"
											title={'Reset to default (' + (cvar.default === '' ? '""' : cvar.default) + ')'}
											onclick={() => resetToDefault(cvar)}
										>
											<RotateCcw class="h-3 w-3" />
										</button>
									{/if}
								</div>
							</div>
						{/each}
						{#if sec.rows.length === 0}
							<div class="px-4 py-4 text-xs text-surface-500">No cvars match your search.</div>
						{/if}
					</div>
				{/if}
			</section>
		{/each}

		<section class="card overflow-hidden">
			<button
				type="button"
				class="flex w-full items-center justify-between gap-3 px-4 py-3 text-left hover:bg-surface-800/30"
				onclick={() => toggleSection('__advanced__')}
			>
				<div class="flex items-center gap-2">
					{#if sectionOpen.__advanced__}
						<ChevronDown class="h-4 w-4 text-surface-500" />
					{:else}
						<ChevronRight class="h-4 w-4 text-surface-500" />
					{/if}
					<h3 class="text-sm font-semibold text-surface-100">Advanced / Custom cvars</h3>
					{#if unknownRows.length > 0}<span class="badge-gray">{unknownRows.length} in file</span>{/if}
					{#if newCvars.length > 0}<span class="badge-yellow">{newCvars.length} new</span>{/if}
				</div>
				<span class="text-xs text-surface-500">custom + unknown</span>
			</button>

			{#if sectionOpen.__advanced__}
				<div class="border-t border-surface-800 p-4">
					<p class="mb-3 text-xs text-surface-500">
						Any cvar in the file that isn't recognized by R3 shows up here. Values preserve round-trip — nothing is lost on save.
					</p>

					{#if unknownRows.length === 0 && newCvars.length === 0}
						<p class="text-xs text-surface-600">No custom cvars. Click "Add cvar" below to add one.</p>
					{/if}

					{#if unknownRows.length > 0}
						<div class="space-y-2">
							{#each unknownRows as k (k)}
								<div class="grid grid-cols-[minmax(0,1fr)_minmax(0,2fr)_auto] items-center gap-2">
									<code class="truncate text-xs text-surface-400">{k}</code>
									<input
										type="text"
										class="input text-xs"
										value={values[k] ?? ''}
										oninput={(e) => setValue(k, e.currentTarget.value)}
									/>
									<button type="button" class="btn-ghost btn-sm" title="Remove from file" onclick={() => removeUnknown(k)}>
										<Trash2 class="h-3 w-3 text-red-400" />
									</button>
								</div>
							{/each}
						</div>
					{/if}

					{#if newCvars.length > 0}
						<div class="mt-3 space-y-2">
							<p class="text-[11px] uppercase tracking-wider text-surface-500">Pending new cvars</p>
							{#each newCvars as nc, i (i)}
								<div class="grid grid-cols-[minmax(0,1fr)_minmax(0,2fr)_auto] items-center gap-2">
									<input
										type="text"
										class="input text-xs font-mono"
										placeholder="cvar_name"
										value={nc.key}
										oninput={(e) => updateNewCvar(i, 'key', e.currentTarget.value)}
									/>
									<input
										type="text"
										class="input text-xs"
										placeholder="value"
										value={nc.value}
										oninput={(e) => updateNewCvar(i, 'value', e.currentTarget.value)}
									/>
									<button type="button" class="btn-ghost btn-sm" onclick={() => removeNewCvar(i)}>
										<Trash2 class="h-3 w-3 text-red-400" />
									</button>
								</div>
							{/each}
						</div>
					{/if}

					<button type="button" class="btn-secondary btn-sm mt-3" onclick={addCustomCvar}>
						<Plus class="h-3 w-3" /> Add cvar
					</button>
				</div>
			{/if}
		</section>
	</div>

	<p class="mt-4 text-xs text-surface-500">
		Changes take effect on next map load (or <code class="text-surface-400">rcon exec server.cfg</code>).
	</p>
{/if}

{#if gearCvarKey}
	<div
		class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4"
		role="dialog"
		aria-modal="true"
		tabindex="-1"
		onclick={closeGearPicker}
		onkeydown={(e) => e.key === 'Escape' && closeGearPicker()}
	>
		<div
			class="w-full max-w-lg rounded-xl border border-surface-700 bg-surface-900 p-5 shadow-2xl"
			role="document"
			onclick={(e) => e.stopPropagation()}
			onkeydown={(e) => e.stopPropagation()}
		>
			<h3 class="mb-2 text-base font-semibold text-surface-100">Gear Picker — {gearCvarKey}</h3>
			<p class="mb-4 text-xs text-surface-500">Tick items to <span class="text-emerald-400">allow</span> them. Unticked items are banned and go into the <code>g_gear</code> string. All ticked = empty g_gear (everything allowed).</p>
			<div class="mb-4 grid grid-cols-2 gap-1">
				{#each GEAR_ITEMS as item (item.code)}
					<label class="flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-xs text-surface-300 hover:bg-surface-800">
						<input
							type="checkbox"
							class="h-4 w-4 accent-accent"
							checked={gearSelection[item.code]}
							onchange={(e) => (gearSelection = { ...gearSelection, [item.code]: e.currentTarget.checked })}
						/>
						<span class="flex-1">{item.label}</span>
						<code class="text-surface-500">{item.code}</code>
					</label>
				{/each}
			</div>
			<div class="flex justify-end gap-2">
				<button class="btn-secondary btn-sm" onclick={closeGearPicker}>Cancel</button>
				<button class="btn-primary btn-sm" onclick={applyGearPicker}>Apply</button>
			</div>
		</div>
	</div>
{/if}
