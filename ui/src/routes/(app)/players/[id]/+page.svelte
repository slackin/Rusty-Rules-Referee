<script>
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { api } from '$lib/api.js';
	import { stripColors, timeAgo, formatDuration, colorize } from '$lib/utils.js';
	import { ArrowLeft, Ban, MessageSquare, Shield, Clock, Globe } from 'lucide-svelte';

	let playerId = $derived(Number($page.params.id));
	let player = $state(null);
	let loading = $state(true);
	let error = $state('');

	// Action modals
	let showKick = $state(false);
	let showBan = $state(false);
	let showMsg = $state(false);
	let actionReason = $state('');
	let actionMsg = $state('');
	let actionDuration = $state(60);
	let actionLoading = $state(false);

	onMount(async () => {
		try {
			player = await api.player(playerId);
		} catch (e) {
			error = e.message;
		}
		loading = false;
	});

	async function kick() {
		actionLoading = true;
		try {
			await api.kickPlayer(player.client.cid ?? playerId, actionReason);
			showKick = false;
		} catch (e) { error = e.message; }
		actionLoading = false;
	}

	async function ban() {
		actionLoading = true;
		try {
			await api.banPlayer(player.client.cid ?? playerId, actionReason, actionDuration);
			showBan = false;
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
					<h1 class="text-2xl font-semibold">{stripColors(player.client.name)}</h1>
					<p class="mt-1 text-sm text-surface-500">Database ID: {player.client.id} · GUID: <span class="font-mono">{player.client.guid ?? '—'}</span></p>
					<div class="mt-3 flex flex-wrap gap-2">
						<span class="badge-blue">{player.client.group_name ?? 'Guest'}</span>
						{#if player.client.ip}
							<span class="badge-gray font-mono">{player.client.ip}</span>
						{/if}
						<span class="badge-gray">Connections: {player.client.connections ?? 0}</span>
					</div>
				</div>
				<div class="flex gap-2">
					<button class="btn-secondary btn-sm" onclick={() => showMsg = true}>
						<MessageSquare class="h-3.5 w-3.5" /> Message
					</button>
					<button class="btn-secondary btn-sm" onclick={() => showKick = true}>
						<Shield class="h-3.5 w-3.5" /> Kick
					</button>
					<button class="btn-danger btn-sm" onclick={() => showBan = true}>
						<Ban class="h-3.5 w-3.5" /> Ban
					</button>
				</div>
			</div>
		</div>

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
							<div class="px-5 py-2.5">
								<div class="flex items-center justify-between">
									<span class="{pen.type === 'Ban' || pen.type === 'TempBan' ? 'badge-red' : 'badge-yellow'}">
										{pen.type}
									</span>
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
