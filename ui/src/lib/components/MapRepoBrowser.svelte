<script>
	/**
	 * MapRepoBrowser — modal that lets admins search the cached external `.pk3`
	 * index (populated by the master's background refresh task) and one-click
	 * import a map onto a game server.
	 *
	 * Props:
	 *   - open: boolean — controls visibility (bindable).
	 *   - serverId: number|null — target server in master mode. Pass null when
	 *     running in standalone/single-server mode and the backend route
	 *     `/server/maps/import` should be used instead.
	 *   - onimported: (filename: string) => void — called after each successful
	 *     import so the parent can refresh the available-maps list and/or open
	 *     a follow-up MapConfig dialog.
	 */
	import { api } from '$lib/api.svelte.js';
	import { Search, Download, X, RefreshCcw, Loader2 } from 'lucide-svelte';

	let { open = $bindable(false), serverId = null, onimported = null } = $props();

	let query = $state('');
	let results = $state([]);
	let total = $state(0);
	let offset = $state(0);
	const LIMIT = 50;
	let loading = $state(false);
	let error = $state('');
	let info = $state('');
	let refreshing = $state(false);
	/** Per-filename import status: 'idle' | 'importing' | 'done' | 'err'. */
	let importState = $state({});
	let lastStatus = $state(null);

	let debounceTimer = null;

	async function runSearch(resetOffset = true) {
		if (resetOffset) offset = 0;
		loading = true;
		error = '';
		try {
			const r = await api.mapRepoSearch(query.trim(), LIMIT, offset);
			results = r.entries || [];
			total = r.total || 0;
		} catch (e) {
			error = e.message;
			results = [];
			total = 0;
		}
		loading = false;
	}

	async function loadStatus() {
		try { lastStatus = await api.mapRepoStatus(); } catch (_) { lastStatus = null; }
	}

	function onQueryInput() {
		if (debounceTimer) clearTimeout(debounceTimer);
		debounceTimer = setTimeout(() => runSearch(true), 250);
	}

	function nextPage() {
		if (offset + LIMIT < total) { offset += LIMIT; runSearch(false); }
	}
	function prevPage() {
		if (offset > 0) { offset = Math.max(0, offset - LIMIT); runSearch(false); }
	}

	async function refreshRepo() {
		refreshing = true;
		info = '';
		try {
			await api.mapRepoRefresh();
			info = 'Refresh started in background. Results will update shortly.';
			// Give the master a few seconds then re-query.
			setTimeout(() => { loadStatus(); runSearch(false); }, 3000);
		} catch (e) { error = e.message; }
		refreshing = false;
	}

	async function doImport(entry) {
		importState = { ...importState, [entry.filename]: 'importing' };
		error = '';
		try {
			const r = serverId
				? await api.serverImportMap(serverId, entry.filename)
				: await api.localImportMap(entry.filename);
			// Backend returns either {response_type: "MapDownloaded", data:{path,size}} or
			// {response_type:"Error", data:{message}} — handle both.
			const rt = r?.response_type;
			if (rt === 'Error') {
				throw new Error(r?.data?.message || 'Import failed');
			}
			importState = { ...importState, [entry.filename]: 'done' };
			if (onimported) onimported(entry.filename);
		} catch (e) {
			importState = { ...importState, [entry.filename]: 'err' };
			error = `Import ${entry.filename} failed: ${e.message}`;
		}
	}

	function fmtSize(n) {
		if (n == null) return '—';
		if (n < 1024) return `${n} B`;
		if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
		if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
		return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
	}

	function close() { open = false; }

	$effect(() => {
		if (open) { loadStatus(); runSearch(true); }
	});
</script>

{#if open}
	<div class="fixed inset-0 bg-black/60 flex items-center justify-center z-[60] backdrop-blur-sm" onclick={close}>
		<div class="bg-zinc-900 border border-zinc-700 rounded-xl shadow-2xl p-5 max-w-4xl w-full mx-4 max-h-[85vh] flex flex-col" onclick={(e) => e.stopPropagation()}>
			<div class="flex items-center justify-between mb-4">
				<div>
					<h3 class="text-lg font-semibold text-white">Browse UrT Map Repository</h3>
					{#if lastStatus}
						<p class="text-xs text-zinc-500 mt-0.5">
							{lastStatus.entry_count?.toLocaleString() || 0} maps cached ·
							{lastStatus.last_refresh ? `updated ${new Date(lastStatus.last_refresh).toLocaleString()}` : 'never refreshed'}
						</p>
					{/if}
				</div>
				<button onclick={close} class="p-1.5 text-zinc-400 hover:text-white"><X size={18} /></button>
			</div>

			<div class="flex gap-2 mb-3">
				<div class="relative flex-1">
					<Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500" />
					<input type="text" bind:value={query} oninput={onQueryInput}
						placeholder="Search map filenames… (e.g. turnpike, ut4_jump_)"
						class="w-full pl-9 pr-3 py-2 bg-zinc-800 border border-zinc-700 rounded-lg text-white placeholder-zinc-500 focus:outline-none focus:border-blue-500 text-sm" />
				</div>
				<button onclick={refreshRepo} disabled={refreshing}
					class="flex items-center gap-1.5 px-3 py-2 bg-zinc-700 text-zinc-200 rounded-lg hover:bg-zinc-600 disabled:opacity-40 text-sm">
					{#if refreshing}<Loader2 size={14} class="animate-spin" />{:else}<RefreshCcw size={14} />{/if}
					Refresh index
				</button>
			</div>

			{#if error}<div class="mb-2 p-2 bg-red-500/20 border border-red-500/40 rounded text-red-300 text-xs">{error}</div>{/if}
			{#if info}<div class="mb-2 p-2 bg-blue-500/20 border border-blue-500/40 rounded text-blue-300 text-xs">{info}</div>{/if}

			<div class="flex-1 overflow-y-auto min-h-0 border border-zinc-700 rounded-lg">
				{#if loading && results.length === 0}
					<div class="p-6 text-center text-sm text-zinc-500">Loading…</div>
				{:else if results.length === 0}
					<div class="p-6 text-center text-sm text-zinc-500">
						No results. {total === 0 && !query ? 'Try clicking "Refresh index".' : ''}
					</div>
				{:else}
					<table class="w-full text-sm">
						<thead class="sticky top-0 bg-zinc-900/95 backdrop-blur">
							<tr class="text-left text-xs uppercase tracking-wider text-zinc-500 border-b border-zinc-700">
								<th class="px-3 py-2">Filename</th>
								<th class="px-3 py-2 w-24">Size</th>
								<th class="px-3 py-2 w-40">Updated</th>
								<th class="px-3 py-2 w-28"></th>
							</tr>
						</thead>
						<tbody>
							{#each results as entry (entry.filename)}
								<tr class="border-b border-zinc-800/50 hover:bg-zinc-800/40">
									<td class="px-3 py-1.5 font-mono text-zinc-200">{entry.filename}</td>
									<td class="px-3 py-1.5 text-zinc-400 tabular-nums">{fmtSize(entry.size)}</td>
									<td class="px-3 py-1.5 text-zinc-500 text-xs">{entry.mtime || '—'}</td>
									<td class="px-3 py-1.5">
										{#if importState[entry.filename] === 'importing'}
											<span class="inline-flex items-center gap-1 text-xs text-zinc-400"><Loader2 size={12} class="animate-spin" /> Importing…</span>
										{:else if importState[entry.filename] === 'done'}
											<span class="text-xs text-green-400">Installed</span>
										{:else}
											<button onclick={() => doImport(entry)}
												class="inline-flex items-center gap-1 px-2.5 py-1 bg-blue-600 text-white rounded-md hover:bg-blue-500 text-xs">
												<Download size={12} /> Import
											</button>
										{/if}
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				{/if}
			</div>

			<div class="mt-3 flex items-center justify-between text-xs text-zinc-500">
				<span>
					{#if total > 0}
						Showing {offset + 1}–{Math.min(offset + LIMIT, total)} of {total.toLocaleString()}
					{/if}
				</span>
				<div class="flex gap-2">
					<button onclick={prevPage} disabled={offset === 0 || loading}
						class="px-2.5 py-1 bg-zinc-800 text-zinc-300 rounded disabled:opacity-40 hover:bg-zinc-700">Prev</button>
					<button onclick={nextPage} disabled={offset + LIMIT >= total || loading}
						class="px-2.5 py-1 bg-zinc-800 text-zinc-300 rounded disabled:opacity-40 hover:bg-zinc-700">Next</button>
				</div>
			</div>
		</div>
	</div>
{/if}
