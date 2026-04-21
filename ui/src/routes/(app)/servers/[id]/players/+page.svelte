<script>
	import { api } from '$lib/api.svelte.js';
	import { page } from '$app/stores';
	import { RefreshCw, VolumeX, Volume2 } from 'lucide-svelte';

	let serverId = $derived(Number($page.params.id));
	let status = $state(null);
	let loading = $state(false);
	let error = $state('');
	let busyCid = $state('');

	async function load() {
		loading = true;
		error = '';
		try {
			status = await api.serverLive(serverId);
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
	<button onclick={load} class="btn btn-sm" disabled={loading}>
		<RefreshCw size={14} class={loading ? 'animate-spin' : ''} /> Refresh
	</button>
</div>

{#if error}<div class="text-red-600 mb-2">{error}</div>{/if}

{#if status}
	<div class="mb-4 text-sm text-gray-500">
		Map: <b>{status.LiveStatus?.map || '—'}</b> ·
		Players: {status.LiveStatus?.player_count || 0}/{status.LiveStatus?.max_clients || 0}
	</div>

	<table class="min-w-full text-sm">
		<thead class="bg-gray-50">
			<tr>
				<th class="p-2 text-left">CID</th>
				<th class="p-2 text-left">Name</th>
				<th class="p-2 text-left">Team</th>
				<th class="p-2 text-right">Score</th>
				<th class="p-2 text-right">Ping</th>
				<th class="p-2 text-left">Actions</th>
			</tr>
		</thead>
		<tbody>
			{#each (status.LiveStatus?.players || []) as p}
				<tr class="border-t hover:bg-gray-50">
					<td class="p-2 font-mono">{p.cid}</td>
					<td class="p-2">{p.name}</td>
					<td class="p-2">{p.team}</td>
					<td class="p-2 text-right">{p.score}</td>
					<td class="p-2 text-right">{p.ping}</td>
					<td class="p-2">
						<button class="btn btn-xs" disabled={busyCid === p.cid} onclick={() => mute(p.cid)}>
							<VolumeX size={14} /> Mute
						</button>
						<button class="btn btn-xs ml-1" disabled={busyCid === p.cid} onclick={() => unmute(p.cid)}>
							<Volume2 size={14} /> Unmute
						</button>
					</td>
				</tr>
			{:else}
				<tr><td colspan="6" class="p-4 text-center text-gray-400">No players online</td></tr>
			{/each}
		</tbody>
	</table>
{/if}
