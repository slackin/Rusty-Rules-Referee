<script>
	/**
	 * MissingMapsDialog — shown before committing a mapcycle. Lists entries that
	 * are referenced in the cycle but aren't installed on the target server, and
	 * annotates each with availability in the cached map repository.
	 *
	 * Props:
	 *   - open (bindable): controls visibility
	 *   - serverId: number|null (null ⇒ standalone mode)
	 *   - missing: Array<{map, repo_filename, repo_size}>
	 *   - onproceed: () => void — called when user clicks "Save anyway"
	 *   - oncancel:  () => void — called when user backs out (close or Cancel)
	 *   - onimported: (filename: string) => void — bubbled up per successful import
	 */
	import { api } from '$lib/api.svelte.js';
	import { X, Download, AlertTriangle, Loader2, CheckCircle2 } from 'lucide-svelte';

	let {
		open = $bindable(false),
		serverId = null,
		missing = [],
		onproceed = null,
		oncancel = null,
		onimported = null,
	} = $props();

	/** Per-map state: 'idle' | 'importing' | 'done' | 'err'. */
	let state = $state({});
	let error = $state('');
	let bulkRunning = $state(false);

	function fmtSize(n) {
		if (n == null) return '';
		if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
		return `${(n / 1024 / 1024).toFixed(1)} MB`;
	}

	async function importOne(m) {
		if (!m.repo_filename) return;
		state = { ...state, [m.map]: 'importing' };
		try {
			const r = serverId
				? await api.serverImportMap(serverId, m.repo_filename)
				: await api.localImportMap(m.repo_filename);
			if (r?.response_type === 'Error') {
				throw new Error(r?.data?.message || 'Import failed');
			}
			state = { ...state, [m.map]: 'done' };
			if (onimported) onimported(m.repo_filename);
		} catch (e) {
			state = { ...state, [m.map]: 'err' };
			error = `${m.map}: ${e.message}`;
		}
	}

	async function importAll() {
		bulkRunning = true;
		error = '';
		for (const m of missing) {
			if (state[m.map] === 'done' || !m.repo_filename) continue;
			await importOne(m);
		}
		bulkRunning = false;
	}

	function cancel() { open = false; if (oncancel) oncancel(); }
	function proceed() { open = false; if (onproceed) onproceed(); }

	let availableCount = $derived(missing.filter(m => !!m.repo_filename).length);
	let allDone = $derived(
		missing.length > 0 && missing.every(m => state[m.map] === 'done' || !m.repo_filename)
	);
</script>

{#if open}
	<div class="fixed inset-0 bg-black/60 flex items-center justify-center z-[70] backdrop-blur-sm" onclick={cancel}>
		<div class="bg-zinc-900 border border-zinc-700 rounded-xl shadow-2xl p-5 max-w-2xl w-full mx-4 max-h-[85vh] flex flex-col" onclick={(e) => e.stopPropagation()}>
			<div class="flex items-start justify-between mb-4">
				<div class="flex gap-3">
					<AlertTriangle size={22} class="text-amber-400 shrink-0 mt-0.5" />
					<div>
						<h3 class="text-lg font-semibold text-white">Missing maps detected</h3>
						<p class="text-xs text-zinc-400 mt-0.5">
							{missing.length} map{missing.length === 1 ? '' : 's'} in this cycle
							{missing.length === 1 ? 'is' : 'are'} not installed on the server.
							{#if availableCount > 0}
								{availableCount} can be imported from the repo.
							{/if}
						</p>
					</div>
				</div>
				<button onclick={cancel} class="p-1.5 text-zinc-400 hover:text-white"><X size={18} /></button>
			</div>

			{#if error}<div class="mb-2 p-2 bg-red-500/20 border border-red-500/40 rounded text-red-300 text-xs">{error}</div>{/if}

			<div class="flex-1 overflow-y-auto min-h-0 border border-zinc-700 rounded-lg">
				<table class="w-full text-sm">
					<thead class="sticky top-0 bg-zinc-900/95">
						<tr class="text-left text-xs uppercase tracking-wider text-zinc-500 border-b border-zinc-700">
							<th class="px-3 py-2">Map</th>
							<th class="px-3 py-2 w-32">Repo size</th>
							<th class="px-3 py-2 w-32 text-right"></th>
						</tr>
					</thead>
					<tbody>
						{#each missing as m (m.map)}
							<tr class="border-b border-zinc-800/50">
								<td class="px-3 py-1.5 font-mono text-zinc-200">{m.map}</td>
								<td class="px-3 py-1.5 text-zinc-500 text-xs">{fmtSize(m.repo_size)}</td>
								<td class="px-3 py-1.5 text-right">
									{#if state[m.map] === 'done'}
										<span class="inline-flex items-center gap-1 text-xs text-green-400"><CheckCircle2 size={12} /> Imported</span>
									{:else if state[m.map] === 'importing'}
										<span class="inline-flex items-center gap-1 text-xs text-zinc-400"><Loader2 size={12} class="animate-spin" /> …</span>
									{:else if m.repo_filename}
										<button onclick={() => importOne(m)}
											class="inline-flex items-center gap-1 px-2.5 py-1 bg-blue-600 text-white rounded-md hover:bg-blue-500 text-xs">
											<Download size={12} /> Import
										</button>
									{:else}
										<span class="text-xs text-zinc-600">not in repo</span>
									{/if}
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>

			<div class="mt-4 flex items-center justify-between gap-2">
				<div>
					{#if availableCount > 0 && !allDone}
						<button onclick={importAll} disabled={bulkRunning}
							class="inline-flex items-center gap-1.5 px-3 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-500 disabled:opacity-40 text-sm">
							{#if bulkRunning}<Loader2 size={14} class="animate-spin" />{:else}<Download size={14} />{/if}
							Import all ({availableCount})
						</button>
					{/if}
				</div>
				<div class="flex gap-2">
					<button onclick={cancel} class="px-3 py-2 bg-zinc-700 text-zinc-200 rounded-lg hover:bg-zinc-600 text-sm">Cancel</button>
					<button onclick={proceed} class="px-3 py-2 bg-amber-600 text-white rounded-lg hover:bg-amber-500 text-sm">
						{allDone ? 'Continue saving' : 'Save anyway'}
					</button>
				</div>
			</div>
		</div>
	</div>
{/if}
