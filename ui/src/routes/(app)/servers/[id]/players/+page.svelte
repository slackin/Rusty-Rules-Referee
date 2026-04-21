<script>
	import { api } from '$lib/api.svelte.js';
	import { page } from '$app/stores';
	import { RefreshCw, VolumeX, Volume2 } from 'lucide-svelte';

	let serverId = $derived(Number($page.params.id));
	let live = $state(null);
	let loading = $state(false);
	let error = $state('');
	let busyCid = $state('');

	async function load() {
		loading = true;
		error = '';
		try {
			const resp = await api.serverLive(serverId);
			// ClientResponse is serialized as { response_type, data }.
			// Fall back to the legacy untagged shape just in case.
			live = resp?.data ?? resp?.LiveStatus ?? null;
		} catch (e) {
			error = e.message || 'Failed to load live status';
		} finally {
			loading = false;
		}
	}

	async function mute(cid) {
		busyCid = cid;
		try { await api.serverPlayerMute(serverId, cid); await load(); }
		catch (e) { error = e.message; }
		finally { busyCid = ''; }
	}
	async function unmute(cid) {
		busyCid = cid;
		try { await api.serverPlayerUnmute(serverId, cid); await load(); }
		catch (e) { error = e.message; }
		finally { busyCid = ''; }
	}

	$effect(() => { load(); });
	// Auto-refresh every 10s
	$effect(() => {
		const h = setInterval(load, 10000);
		return () => clearInterval(h);
	});
</script>

<div class="flex justify-between items-center mb-3">
	<h2 class="text-xl font-semibold">Live Players</h2>
	<button onclick={load} class="btn btn-secondary btn-sm" disabled={loading}>
		<RefreshCw size={14} class={loading ? 'animate-spin' : ''} /> Refresh
	</button>
</div>

{#if error}<div class="text-red-400 mb-2">{error}</div>{/if}

{#if live}
	<div class="mb-4 text-sm text-surface-400">
		Map: <b class="text-surface-200">{live.map || '—'}</b> ·
		Players: {live.player_count ?? 0}/{live.max_clients ?? 0}
	</div>

	<div class="card overflow-hidden">
		<table class="w-full text-sm">
			<thead>
				<tr class="border-b border-surface-800 text-left text-xs font-medium uppercase tracking-wider text-surface-500">
					<th class="px-4 py-3">CID</th>
					<th class="px-4 py-3">Name</th>
					<th class="px-4 py-3">Team</th>
					<th class="px-4 py-3 text-right">Score</th>
					<th class="px-4 py-3 text-right">Ping</th>
					<th class="px-4 py-3">Actions</th>
				</tr>
			</thead>
			<tbody class="divide-y divide-surface-800/50">
				{#each (live.players || []) as p}
					<tr class="hover:bg-surface-800/30 transition-colors">
						<td class="px-4 py-2 font-mono text-surface-300">{p.cid}</td>
						<td class="px-4 py-2 text-surface-200">{p.name}</td>
						<td class="px-4 py-2 text-surface-400">{p.team ?? '—'}</td>
						<td class="px-4 py-2 text-right text-surface-300">{p.score}</td>
						<td class="px-4 py-2 text-right text-surface-300">{p.ping}</td>
						<td class="px-4 py-2">
							<button class="btn btn-secondary btn-sm" disabled={busyCid === p.cid} onclick={() => mute(p.cid)}>
								<VolumeX size={14} /> Mute
							</button>
							<button class="btn btn-secondary btn-sm ml-1" disabled={busyCid === p.cid} onclick={() => unmute(p.cid)}>
								<Volume2 size={14} /> Unmute
							</button>
						</td>
					</tr>
				{:else}
					<tr><td colspan="6" class="px-4 py-6 text-center text-surface-500">No players online</td></tr>
				{/each}
			</tbody>
		</table>
	</div>
{/if}
