<script>
	/**
	 * MapConfigCreateDialog — lightweight "do you want a MapConfig for this?"
	 * prompt, shown right after a successful import. It pre-fills `map_name`
	 * from the pk3 filename and guesses `gametype` from common name prefixes
	 * (`ut4_jump_` → 9, `_ctf` → 7, `_bomb` → 8, default FFA). The admin may
	 * tweak any field and save, or skip (optionally for the rest of the
	 * session via the "don't ask again" toggle).
	 *
	 * Props:
	 *   - open (bindable)
	 *   - filename: string — the `.pk3` file that was just imported
	 *   - oncreated: (config) => void — after the POST succeeds
	 *   - onskip:    () => void
	 */
	import { api } from '$lib/api.svelte.js';
	import { X, FileCog, Loader2 } from 'lucide-svelte';

	let { open = $bindable(false), filename = '', oncreated = null, onskip = null } = $props();

	const GAMETYPES = [
		{ v: '0', l: 'Free For All (FFA)' },
		{ v: '1', l: 'Last Man Standing' },
		{ v: '3', l: 'Team Death Match' },
		{ v: '4', l: 'Team Survivor' },
		{ v: '5', l: 'Follow The Leader' },
		{ v: '6', l: 'Capture & Hold' },
		{ v: '7', l: 'Capture The Flag' },
		{ v: '8', l: 'Bomb Mode' },
		{ v: '9', l: 'Jump Mode' },
		{ v: '10', l: 'Freeze Tag' },
		{ v: '11', l: 'Gun Game' },
	];

	let mapName = $state('');
	let gametype = $state('0');
	let timelimit = $state('');
	let dontAsk = $state(false);
	let saving = $state(false);
	let error = $state('');

	function guessGametype(name) {
		const lc = name.toLowerCase();
		if (lc.startsWith('ut4_jump') || lc.includes('_jump_')) return '9';
		if (lc.includes('_ctf')) return '7';
		if (lc.includes('_bomb')) return '8';
		if (lc.includes('_ft')) return '10';
		if (lc.includes('_cah')) return '6';
		if (lc.includes('_ts')) return '4';
		return '0';
	}

	$effect(() => {
		if (open && filename) {
			const stem = filename.replace(/\.pk3$/i, '');
			mapName = stem;
			gametype = guessGametype(stem);
			error = '';
			saving = false;
		}
	});

	async function create() {
		if (!mapName.trim()) { error = 'Map name required'; return; }
		saving = true;
		error = '';
		const cfg = {
			id: 0,
			map_name: mapName.trim(),
			gametype,
			capturelimit: null,
			timelimit: timelimit ? Number(timelimit) : null,
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
		};
		try {
			const r = await api.createMapConfig(cfg);
			if (oncreated) oncreated(r);
			open = false;
		} catch (e) { error = e.message; }
		saving = false;
	}

	function skip() {
		if (dontAsk) {
			try { sessionStorage.setItem('r3.skipMapConfigPrompt', '1'); } catch (_) {}
		}
		open = false;
		if (onskip) onskip();
	}
</script>

{#if open}
	<div class="fixed inset-0 bg-black/60 flex items-center justify-center z-[80] backdrop-blur-sm" onclick={skip}>
		<div class="bg-zinc-900 border border-zinc-700 rounded-xl shadow-2xl p-5 max-w-md w-full mx-4" onclick={(e) => e.stopPropagation()}>
			<div class="flex items-start justify-between mb-4">
				<div class="flex gap-3">
					<FileCog size={22} class="text-blue-400 shrink-0 mt-0.5" />
					<div>
						<h3 class="text-lg font-semibold text-white">Create MapConfig?</h3>
						<p class="text-xs text-zinc-400 mt-0.5">Set per-map rules for <code class="text-zinc-300">{filename}</code>.</p>
					</div>
				</div>
				<button onclick={skip} class="p-1.5 text-zinc-400 hover:text-white"><X size={18} /></button>
			</div>

			{#if error}<div class="mb-2 p-2 bg-red-500/20 border border-red-500/40 rounded text-red-300 text-xs">{error}</div>{/if}

			<div class="space-y-3">
				<div>
					<label class="block text-xs text-zinc-400 mb-1" for="mcd-name">Map name</label>
					<input id="mcd-name" type="text" bind:value={mapName}
						class="w-full px-3 py-2 bg-zinc-800 border border-zinc-700 rounded-lg text-white font-mono text-sm focus:outline-none focus:border-blue-500" />
				</div>
				<div>
					<label class="block text-xs text-zinc-400 mb-1" for="mcd-gt">Gametype</label>
					<select id="mcd-gt" bind:value={gametype}
						class="w-full px-3 py-2 bg-zinc-800 border border-zinc-700 rounded-lg text-white text-sm focus:outline-none focus:border-blue-500">
						{#each GAMETYPES as g}
							<option value={g.v}>{g.l}</option>
						{/each}
					</select>
				</div>
				<div>
					<label class="block text-xs text-zinc-400 mb-1" for="mcd-tl">Time limit (optional)</label>
					<input id="mcd-tl" type="number" min="0" bind:value={timelimit} placeholder="—"
						class="w-full px-3 py-2 bg-zinc-800 border border-zinc-700 rounded-lg text-white text-sm focus:outline-none focus:border-blue-500" />
				</div>
			</div>

			<label class="flex items-center gap-2 mt-4 text-xs text-zinc-400">
				<input type="checkbox" bind:checked={dontAsk} class="rounded" />
				Don't ask again this session
			</label>

			<div class="mt-4 flex justify-end gap-2">
				<button onclick={skip} class="px-3 py-2 bg-zinc-700 text-zinc-200 rounded-lg hover:bg-zinc-600 text-sm">Skip</button>
				<button onclick={create} disabled={saving || !mapName.trim()}
					class="inline-flex items-center gap-1.5 px-3 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-500 disabled:opacity-40 text-sm">
					{#if saving}<Loader2 size={14} class="animate-spin" />{/if}
					Create MapConfig
				</button>
			</div>
		</div>
	</div>
{/if}
