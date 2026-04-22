<script>
	// Live g_gear editor — reads current value from the server via RCON,
	// lets the admin toggle items, and pushes each change immediately.
	//
	// UrT semantics: g_gear is a BAN list. Letters in the cvar DISABLE the
	// corresponding item. Empty g_gear = everything allowed. We invert the
	// checkboxes in the UI so checked = allowed.
	import { onMount } from 'svelte';
	import { api } from '$lib/api.svelte.js';
	import { GEAR_ITEMS } from '$lib/urt-cvars.js';

	// If `serverId` is provided we use the master-mode per-server endpoints,
	// otherwise the standalone endpoints.
	let { serverId = null } = $props();

	let loading = $state(true);
	let saving = $state(false);
	let error = $state('');
	let status = $state('');
	let gearString = $state(''); // current server-side g_gear (ban list)
	// selection[code] = allowed? (true = checked in UI)
	let selection = $state({});

	const codeOrder = GEAR_ITEMS.map((g) => g.code);

	function applyStringToSelection(str) {
		const banned = new Set((str || '').split(''));
		const next = {};
		for (const item of GEAR_ITEMS) next[item.code] = !banned.has(item.code);
		selection = next;
	}

	function selectionToBanString() {
		return codeOrder.filter((c) => !selection[c]).join('');
	}

	async function readCvar(name) {
		if (serverId != null) {
			const resp = await api.serverGetCvar(serverId, name);
			// master returns { response_type: 'Ok', data: { name, value, raw } }
			// or { response_type: 'Error', data: { message } }
			if (resp?.response_type === 'Error') throw new Error(resp?.data?.message || 'Read failed');
			return resp?.data?.value ?? '';
		} else {
			const resp = await api.getCvar(name);
			return resp?.value ?? '';
		}
	}

	async function writeCvar(name, value) {
		if (serverId != null) {
			const resp = await api.serverSetCvar(serverId, name, value);
			if (resp?.response_type === 'Error') throw new Error(resp?.data?.message || 'Write failed');
			return resp?.data?.value ?? value;
		} else {
			const resp = await api.setCvar(name, value);
			return resp?.value ?? value;
		}
	}

	async function refresh() {
		loading = true;
		error = '';
		try {
			const val = await readCvar('g_gear');
			gearString = val;
			applyStringToSelection(val);
			status = `Loaded g_gear="${val || '(empty)'}"`;
		} catch (e) {
			error = String(e?.message ?? e);
		} finally {
			loading = false;
		}
	}

	async function toggleItem(code) {
		if (saving) return;
		const newSel = { ...selection, [code]: !selection[code] };
		selection = newSel;
		const newBan = codeOrder.filter((c) => !newSel[c]).join('');
		saving = true;
		error = '';
		try {
			await writeCvar('g_gear', newBan);
			gearString = newBan;
			status = `Set g_gear="${newBan || '(empty — all allowed)'}"`;
		} catch (e) {
			error = String(e?.message ?? e);
			// Revert UI on failure.
			applyStringToSelection(gearString);
		} finally {
			saving = false;
		}
	}

	async function allowAll() {
		if (saving) return;
		saving = true;
		error = '';
		try {
			await writeCvar('g_gear', '');
			gearString = '';
			applyStringToSelection('');
			status = 'Reset — all items allowed';
		} catch (e) {
			error = String(e?.message ?? e);
		} finally {
			saving = false;
		}
	}

	async function denyAll() {
		if (saving) return;
		const all = codeOrder.join('');
		saving = true;
		error = '';
		try {
			await writeCvar('g_gear', all);
			gearString = all;
			applyStringToSelection(all);
			status = 'All items banned';
		} catch (e) {
			error = String(e?.message ?? e);
		} finally {
			saving = false;
		}
	}

	onMount(refresh);
</script>

<div class="space-y-4">
	<header class="flex flex-wrap items-center justify-between gap-2">
		<div>
			<h2 class="text-lg font-semibold text-zinc-100">Live g_gear editor</h2>
			<p class="text-xs text-zinc-500">
				Changes are pushed to the game server immediately via RCON.
				Checked = allowed; unchecked items go into <code>g_gear</code> (the UrT ban list).
			</p>
		</div>
		<div class="flex gap-2">
			<button
				type="button"
				class="rounded border border-zinc-700 px-3 py-1 text-xs text-zinc-300 hover:bg-zinc-800 disabled:opacity-50"
				disabled={saving || loading}
				onclick={refresh}
			>Reload</button>
			<button
				type="button"
				class="rounded border border-emerald-700/60 px-3 py-1 text-xs text-emerald-300 hover:bg-emerald-900/30 disabled:opacity-50"
				disabled={saving || loading}
				onclick={allowAll}
			>Allow all</button>
			<button
				type="button"
				class="rounded border border-red-700/60 px-3 py-1 text-xs text-red-300 hover:bg-red-900/30 disabled:opacity-50"
				disabled={saving || loading}
				onclick={denyAll}
			>Ban all</button>
		</div>
	</header>

	{#if error}
		<div class="rounded border border-red-700/50 bg-red-900/30 px-3 py-2 text-sm text-red-200">{error}</div>
	{:else if status}
		<div class="rounded border border-zinc-800 bg-zinc-900/40 px-3 py-2 text-xs text-zinc-400">{status}</div>
	{/if}

	{#if loading}
		<div class="text-sm text-zinc-500">Loading current g_gear…</div>
	{:else}
		<div class="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-4">
			{#each GEAR_ITEMS as item}
				<label class="flex items-center gap-2 rounded border border-zinc-800 px-2 py-1.5 text-xs hover:border-zinc-700">
					<input type="checkbox" class="accent-blue-500"
						checked={!!selection[item.code]}
						disabled={saving}
						onchange={() => toggleItem(item.code)} />
					<span class="inline-block w-5 text-center font-mono text-zinc-300">{item.code}</span>
					<span class="text-zinc-400">{item.label}</span>
				</label>
			{/each}
		</div>
		<div class="mt-2 text-xs text-zinc-500">
			Current <code>g_gear</code>: <code class="text-zinc-300">{gearString || '(empty — all allowed)'}</code>
			{#if saving}<span class="ml-2 text-blue-400">saving…</span>{/if}
		</div>
	{/if}
</div>
