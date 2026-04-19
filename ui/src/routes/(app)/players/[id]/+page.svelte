<script>
	import { onMount, onDestroy } from 'svelte';
	import { page } from '$app/stores';
	import { api } from '$lib/api.svelte.js';
	import { onEvent } from '$lib/ws.js';
	import { stripColors, timeAgo, formatDuration, colorize, getGearStyle, pingColor, teamInfo } from '$lib/utils.js';
	import { ArrowLeft, Ban, MessageSquare, Shield, Clock, Globe, ShieldCheck, UserCog, Wifi, Crosshair, Swords, Activity, VolumeX } from 'lucide-svelte';

	let playerId = $derived(Number($page.params.id));
	let player = $state(null);
	let groups = $state([]);
	let loading = $state(true);
	let error = $state('');

	// Action modals
	let showKick = $state(false);
	let showBan = $state(false);
	let showMsg = $state(false);
	let showMute = $state(false);
	let showGroup = $state(false);
	let showSettings = $state(false);
	let actionReason = $state('');
	let actionMsg = $state('');
	let actionDuration = $state(60);
	let muteDuration = $state(600);
	let actionLoading = $state(false);
	let selectedGroupId = $state(null);

	let hasActiveBans = $derived(
		player?.penalties?.some(p => (p.type === 'Ban' || p.type === 'TempBan') && !p.inactive) ?? false
	);
	let isOnline = $derived(player?.client?.cid != null);
	let live = $derived(player?.live);
	let team = $derived(teamInfo(player?.client?.team));

	// Auto-refresh timer for live data (score, ping, gear)
	let refreshTimer = null;
	let unsubWs = null;

	async function refreshPlayer() {
		try {
			const data = await api.player(playerId);
			player = data;
		} catch (e) {
			// Player may have disconnected
			console.warn('[player] refresh failed:', e);
		}
	}

	onMount(async () => {
		try {
			const [playerData, groupData] = await Promise.all([
				api.player(playerId),
				api.groups()
			]);
			player = playerData;
			groups = groupData;
		} catch (e) {
			error = e.message;
		}
		loading = false;

		// Poll live data every 10s for score/ping updates
		refreshTimer = setInterval(refreshPlayer, 10_000);

		// Subscribe to WS events relevant to this player
		unsubWs = onEvent((evt) => {
			if (evt.client_id === playerId || evt.target_id === playerId) {
				// Event involves this player — schedule quick refresh
				scheduleQuickRefresh();
			}
			// Player disconnected
			if (evt.type === 'EVT_CLIENT_DISCONNECT' && evt.client_id === playerId && player) {
				player = { ...player, client: { ...player.client, cid: null }, live: null };
			}
		});
	});

	onDestroy(() => {
		if (refreshTimer) clearInterval(refreshTimer);
		if (unsubWs) unsubWs();
	});

	let quickTimer = null;
	function scheduleQuickRefresh() {
		if (quickTimer) return;
		quickTimer = setTimeout(() => {
			quickTimer = null;
			refreshPlayer();
		}, 600);
	}

	async function kick() {
		actionLoading = true;
		try {
			await api.kickPlayer(player.client.cid ?? playerId, actionReason);
			showKick = false;
			setTimeout(refreshPlayer, 1000);
		} catch (e) { error = e.message; }
		actionLoading = false;
	}

	async function ban() {
		actionLoading = true;
		try {
			await api.banPlayer(player.client.cid ?? playerId, actionReason, actionDuration);
			showBan = false;
			setTimeout(refreshPlayer, 1000);
		} catch (e) { error = e.message; }
		actionLoading = false;
	}

	async function sendMessage() {
		actionLoading = true;
		try {
			await api.messagePlayer(player.client.cid ?? playerId, actionMsg);
			showMsg = false;
			actionMsg = '';
		} catch (e) { error = e.message; }
		actionLoading = false;
	}

	async function mute() {
		actionLoading = true;
		try {
			await api.mutePlayer(player.client.cid ?? playerId, muteDuration, actionReason);
			showMute = false;
			actionReason = '';
			setTimeout(refreshPlayer, 1000);
		} catch (e) { error = e.message; }
		actionLoading = false;
	}

	async function unmute() {
		actionLoading = true;
		try {
			await api.unmutePlayer(player.client.cid ?? playerId);
			setTimeout(refreshPlayer, 1000);
		} catch (e) { error = e.message; }
		actionLoading = false;
	}

	async function changeGroup() {
		if (!selectedGroupId) return;
		actionLoading = true;
		try {
			const result = await api.updatePlayerGroup(playerId, Number(selectedGroupId));
			player.client.group_name = result.group_name;
			player.client.group_bits = result.group_bits;
			showGroup = false;
		} catch (e) { error = e.message; }
		actionLoading = false;
	}

	async function unban() {
		if (!confirm('Disable all active bans/tempbans for this player?')) return;
		actionLoading = true;
		try {
			await api.disablePenalty(playerId);
			const updated = await api.player(playerId);
			player.penalties = updated.penalties;
		} catch (e) { error = e.message; }
		actionLoading = false;
	}
</script>

<div class="space-y-6 animate-fade-in">
	<a href="/players" class="inline-flex items-center gap-2 text-sm text-surface-500 hover:text-surface-300 transition-colors">
		<ArrowLeft class="h-4 w-4" /> Back to Players
	</a>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
		</div>
	{:else if error}
		<div class="card p-6 text-center text-red-400">{error}</div>
	{:else if player}
		<!-- Header -->
		<div class="card p-6">
			<div class="flex flex-wrap items-start justify-between gap-4">
				<div>
					<div class="flex items-center gap-3">
						<h1 class="text-2xl font-semibold">{stripColors(player.client.name)}</h1>
						{#if isOnline}
							<span class="inline-flex items-center gap-1.5 rounded-full bg-green-500/15 border border-green-500/30 px-2.5 py-0.5 text-xs font-medium text-green-400">
								<span class="h-1.5 w-1.5 rounded-full bg-green-400 animate-pulse"></span> Online
							</span>
						{:else}
							<span class="inline-flex items-center gap-1.5 rounded-full bg-surface-500/15 border border-surface-500/30 px-2.5 py-0.5 text-xs font-medium text-surface-500">
								Offline
							</span>
						{/if}
					</div>
					{#if isOnline && live?.current_name && stripColors(live.current_name) !== stripColors(player.client.name)}
						<p class="mt-1 text-sm text-surface-400">Current Nick: <span class="text-surface-200">{stripColors(live.current_name)}</span></p>
					{/if}
					<p class="mt-1 text-sm text-surface-500">Database ID: {player.client.id} · GUID: <span class="font-mono">{player.client.guid ?? '—'}</span></p>
					<div class="mt-3 flex flex-wrap gap-2">
						<span class="badge-blue">{player.client.group_name ?? 'Guest'}</span>
						{#if isOnline}
							<span class="inline-flex items-center gap-1 rounded border px-2 py-0.5 text-xs font-medium {team.bg} {team.color}">
								{team.label}
							</span>
						{/if}
						{#if player.client.ip}
							<span class="badge-gray font-mono">{player.client.ip}</span>
						{/if}
						{#if live?.auth || player.client.auth}
							<span class="inline-flex items-center gap-1.5 rounded border border-purple-500/30 bg-purple-500/10 px-2 py-0.5 text-xs font-medium text-purple-400">
								<ShieldCheck class="h-3 w-3" /> {live?.auth ?? player.client.auth}
							</span>
						{/if}
						{#if live?.armband}
							<span class="inline-flex items-center gap-1.5 rounded border border-surface-700 bg-surface-800/50 px-2 py-0.5 text-xs font-medium text-surface-300">
								<span class="h-3 w-3 rounded-full border border-surface-600" style="background-color: rgb({live.armband})"></span>
								Armband
							</span>
						{/if}
					</div>
				</div>
				<div class="flex flex-wrap gap-2">
					{#if isOnline}
						<button class="btn-secondary btn-sm" onclick={() => showGroup = true}>
							<UserCog class="h-3.5 w-3.5" /> Change Group
						</button>
						<button class="btn-secondary btn-sm" onclick={() => showMsg = true}>
							<MessageSquare class="h-3.5 w-3.5" /> Message
						</button>
						<button class="btn-secondary btn-sm" onclick={() => showMute = true}>
							<VolumeX class="h-3.5 w-3.5" /> Mute
						</button>
						<button class="btn-secondary btn-sm text-yellow-400 hover:text-yellow-300" onclick={unmute} disabled={actionLoading}>
							<VolumeX class="h-3.5 w-3.5" /> Unmute
						</button>
						<button class="btn-secondary btn-sm" onclick={() => showKick = true}>
							<Shield class="h-3.5 w-3.5" /> Kick
						</button>
					{/if}
					{#if hasActiveBans}
						<button class="btn-secondary btn-sm text-green-400 hover:text-green-300" onclick={unban} disabled={actionLoading}>
							<ShieldCheck class="h-3.5 w-3.5" /> Unban
						</button>
					{/if}
					{#if isOnline}
						<button class="btn-danger btn-sm" onclick={() => showBan = true}>
							<Ban class="h-3.5 w-3.5" /> Ban
						</button>
					{/if}
				</div>
			</div>
		</div>

		<!-- Live Status Bar (only when online) -->
		{#if isOnline && live}
			<div class="grid grid-cols-2 gap-4 sm:grid-cols-4">
				<div class="card p-4 flex items-center gap-3">
					<div class="flex h-10 w-10 items-center justify-center rounded-lg bg-accent/10 text-accent">
						<Crosshair class="h-5 w-5" />
					</div>
					<div>
						<p class="text-xs text-surface-500">Score</p>
						<p class="text-xl font-bold tabular-nums">{live.score ?? '—'}</p>
					</div>
				</div>
				<div class="card p-4 flex items-center gap-3">
					<div class="flex h-10 w-10 items-center justify-center rounded-lg bg-accent/10 text-accent">
						<Wifi class="h-5 w-5" />
					</div>
					<div>
						<p class="text-xs text-surface-500">Ping</p>
						<p class="text-xl font-bold tabular-nums {pingColor(live.ping)}">{live.ping ?? '—'}<span class="text-sm font-normal text-surface-500">ms</span></p>
					</div>
				</div>
				<div class="card p-4 flex items-center gap-3">
					<div class="flex h-10 w-10 items-center justify-center rounded-lg bg-accent/10 text-accent">
						<Swords class="h-5 w-5" />
					</div>
					<div>
						<p class="text-xs text-surface-500">Slot</p>
						<p class="text-xl font-bold tabular-nums">#{player.client.cid}</p>
					</div>
				</div>
				<div class="card p-4 flex items-center gap-3">
					<div class="flex h-10 w-10 items-center justify-center rounded-lg bg-accent/10 text-accent">
						<Activity class="h-5 w-5" />
					</div>
					<div>
						<p class="text-xs text-surface-500">Gear</p>
						<p class="text-xl font-bold font-mono tracking-wider">{live.gear ?? '—'}</p>
					</div>
				</div>
			</div>
		{/if}

		<!-- Loadout (only when online and gear available) -->
		{#if live?.loadout?.length > 0}
			<div class="card">
				<div class="border-b border-surface-800 px-5 py-4">
					<h2 class="text-sm font-medium text-surface-300">Current Loadout</h2>
				</div>
				<div class="grid grid-cols-1 gap-3 p-5 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
					{#each live.loadout as item}
						{@const style = getGearStyle(item.category)}
						<div class="flex items-center gap-3 rounded-lg border p-3 {style.bg}">
							<span class="text-2xl">{style.icon}</span>
							<div class="min-w-0">
								<p class="text-sm font-medium text-surface-200 truncate">{item.name}</p>
								<p class="text-xs {style.color}">{item.slot} · {item.category}</p>
							</div>
						</div>
					{/each}
				</div>
			</div>
		{/if}

		<div class="grid grid-cols-1 gap-6 lg:grid-cols-2">
			<!-- Aliases -->
			<div class="card">
				<div class="border-b border-surface-800 px-5 py-4">
					<h2 class="text-sm font-medium text-surface-300">Aliases</h2>
				</div>
				{#if player.aliases?.length > 0}
					<div class="max-h-64 overflow-y-auto divide-y divide-surface-800/50">
						{#each player.aliases as alias}
							<div class="flex items-center justify-between px-5 py-2.5">
								<span class="text-sm text-surface-200">{stripColors(alias.name)}</span>
								<span class="text-xs text-surface-500">used {alias.num_used ?? 0}× · {timeAgo(alias.time_add)}</span>
							</div>
						{/each}
					</div>
				{:else}
					<div class="px-5 py-6 text-center text-sm text-surface-500">No aliases</div>
				{/if}
			</div>

			<!-- Penalties -->
			<div class="card">
				<div class="border-b border-surface-800 px-5 py-4">
					<h2 class="text-sm font-medium text-surface-300">Penalties</h2>
				</div>
				{#if player.penalties?.length > 0}
					<div class="max-h-64 overflow-y-auto divide-y divide-surface-800/50">
						{#each player.penalties as pen}
							<div class="px-5 py-2.5 {pen.inactive ? 'opacity-50' : ''}">
								<div class="flex items-center justify-between">
									<div class="flex items-center gap-2">
										<span class="{pen.type === 'Ban' || pen.type === 'TempBan' ? 'badge-red' : 'badge-yellow'}">
											{pen.type}
										</span>
										{#if pen.inactive}
											<span class="text-xs text-surface-500 italic">inactive</span>
										{/if}
									</div>
									<span class="text-xs text-surface-500">{timeAgo(pen.time_add)}</span>
								</div>
								<p class="mt-1 text-xs text-surface-400">{pen.reason ?? 'No reason'}</p>
								{#if pen.duration}
									<p class="text-xs text-surface-500">{formatDuration(pen.duration)}</p>
								{/if}
							</div>
						{/each}
					</div>
				{:else}
					<div class="px-5 py-6 text-center text-sm text-surface-500">No penalties</div>
				{/if}
			</div>
		</div>

		<!-- XLR Stats -->
		{#if player.xlr_stats}
			<div class="card p-5">
				<h2 class="mb-4 text-sm font-medium text-surface-300">XLR Statistics</h2>
				<div class="grid grid-cols-2 gap-4 sm:grid-cols-4 lg:grid-cols-6">
					{#each Object.entries(player.xlr_stats) as [key, val]}
						<div>
							<p class="text-xs uppercase tracking-wider text-surface-500">{key.replace(/_/g, ' ')}</p>
							<p class="mt-0.5 text-lg font-semibold text-surface-200">{typeof val === 'number' ? val.toLocaleString() : val}</p>
						</div>
					{/each}
				</div>
			</div>
		{/if}

		<!-- Client Settings (collapsible, only when online) -->
		{#if live?.settings}
			<div class="card">
				<button class="w-full border-b border-surface-800 px-5 py-4 text-left flex items-center justify-between" onclick={() => showSettings = !showSettings}>
					<h2 class="text-sm font-medium text-surface-300">Client Settings</h2>
					<span class="text-xs text-surface-500">{showSettings ? '▲' : '▼'}</span>
				</button>
				{#if showSettings}
					<div class="grid grid-cols-2 gap-3 p-5 sm:grid-cols-3 lg:grid-cols-4">
						{#each Object.entries(live.settings) as [key, val]}
							<div class="rounded-lg border border-surface-800 bg-surface-900/50 px-3 py-2">
								<p class="text-xs text-surface-500 font-mono">{key}</p>
								<p class="text-sm text-surface-200 font-mono">{val}</p>
							</div>
						{/each}
					</div>
				{/if}
			</div>
		{/if}
	{/if}
</div>

<!-- Kick Modal -->
{#if showKick}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onclick={() => showKick = false}>
		<div class="card w-full max-w-md p-6 animate-slide-up" onclick={(e) => e.stopPropagation()}>
			<h3 class="text-lg font-semibold">Kick Player</h3>
			<p class="mt-1 text-sm text-surface-500">Kick {stripColors(player.client.name)} from the server</p>
			<input class="input mt-4" bind:value={actionReason} placeholder="Reason (optional)" />
			<div class="mt-4 flex justify-end gap-2">
				<button class="btn-secondary btn-sm" onclick={() => showKick = false}>Cancel</button>
				<button class="btn-primary btn-sm" onclick={kick} disabled={actionLoading}>Kick</button>
			</div>
		</div>
	</div>
{/if}

<!-- Ban Modal -->
{#if showBan}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onclick={() => showBan = false}>
		<div class="card w-full max-w-md p-6 animate-slide-up" onclick={(e) => e.stopPropagation()}>
			<h3 class="text-lg font-semibold">Ban Player</h3>
			<p class="mt-1 text-sm text-surface-500">Ban {stripColors(player.client.name)}</p>
			<input class="input mt-4" bind:value={actionReason} placeholder="Reason" />
			<div class="mt-3">
				<label class="mb-1 block text-xs text-surface-500">Duration (minutes, 0 = permanent)</label>
				<input type="number" class="input" bind:value={actionDuration} min="0" />
			</div>
			<div class="mt-4 flex justify-end gap-2">
				<button class="btn-secondary btn-sm" onclick={() => showBan = false}>Cancel</button>
				<button class="btn-danger btn-sm" onclick={ban} disabled={actionLoading}>Ban</button>
			</div>
		</div>
	</div>
{/if}

<!-- Message Modal -->
{#if showMsg}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onclick={() => showMsg = false}>
		<div class="card w-full max-w-md p-6 animate-slide-up" onclick={(e) => e.stopPropagation()}>
			<h3 class="text-lg font-semibold">Send Message</h3>
			<input class="input mt-4" bind:value={actionMsg} placeholder="Message to player" />
			<div class="mt-4 flex justify-end gap-2">
				<button class="btn-secondary btn-sm" onclick={() => showMsg = false}>Cancel</button>
				<button class="btn-primary btn-sm" onclick={sendMessage} disabled={actionLoading || !actionMsg}>Send</button>
			</div>
		</div>
	</div>
{/if}

<!-- Mute Modal -->
{#if showMute}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onclick={() => showMute = false}>
		<div class="card w-full max-w-md p-6 animate-slide-up" onclick={(e) => e.stopPropagation()}>
			<h3 class="text-lg font-semibold">Mute Player</h3>
			<p class="mt-1 text-sm text-surface-500">Mute {stripColors(player.client.name)}</p>
			<input class="input mt-4" bind:value={actionReason} placeholder="Reason (optional)" />
			<div class="mt-3">
				<label class="mb-1 block text-xs text-surface-500">Duration (seconds)</label>
				<input type="number" class="input" bind:value={muteDuration} min="1" />
			</div>
			<div class="mt-4 flex justify-end gap-2">
				<button class="btn-secondary btn-sm" onclick={() => showMute = false}>Cancel</button>
				<button class="btn-primary btn-sm" onclick={mute} disabled={actionLoading}>Mute</button>
			</div>
		</div>
	</div>
{/if}

<!-- Change Group Modal -->
{#if showGroup}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onclick={() => showGroup = false}>
		<div class="card w-full max-w-md p-6 animate-slide-up" onclick={(e) => e.stopPropagation()}>
			<h3 class="text-lg font-semibold">Change Group</h3>
			<p class="mt-1 text-sm text-surface-500">Set permission group for {stripColors(player.client.name)}</p>
			<select class="input mt-4" bind:value={selectedGroupId}>
				<option value={null} disabled>Select a group…</option>
				{#each groups as g}
					<option value={g.id}>{g.name} (Level {g.level})</option>
				{/each}
			</select>
			<div class="mt-4 flex justify-end gap-2">
				<button class="btn-secondary btn-sm" onclick={() => showGroup = false}>Cancel</button>
				<button class="btn-primary btn-sm" onclick={changeGroup} disabled={actionLoading || !selectedGroupId}>Save</button>
			</div>
		</div>
	</div>
{/if}
