<script>
	import { api } from '$lib/api.svelte.js';
	import { HardDrive, RefreshCw, Trash2, Cpu, Wifi, WifiOff } from 'lucide-svelte';

	let hubs = $state([]);
	let loading = $state(true);
	let error = $state('');
	let refreshing = $state(false);
	let deleteTarget = $state(null);
	let deleting = $state(false);

	async function load() {
		try {
			hubs = await api.hubs();
			error = '';
		} catch (e) {
			error = e.message || 'Failed to load hubs';
		}
		loading = false;
	}

	async function refresh() {
		refreshing = true;
		await load();
		refreshing = false;
	}

	async function confirmDelete() {
		if (!deleteTarget) return;
		deleting = true;
		try {
			await api.deleteHub(deleteTarget.id);
			hubs = hubs.filter(h => h.id !== deleteTarget.id);
			deleteTarget = null;
		} catch (e) {
			error = e.message || 'Failed to delete hub';
		}
		deleting = false;
	}

	function fmtAge(ts) {
		if (!ts) return 'never';
		const secs = Math.floor((Date.now() - new Date(ts).getTime()) / 1000);
		if (secs < 60) return `${secs}s ago`;
		if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
		if (secs < 86400) return `${Math.floor(secs / 3600)}h ago`;
		return `${Math.floor(secs / 86400)}d ago`;
	}

	function isOnline(hub) {
		if (!hub.last_seen) return false;
		return (Date.now() - new Date(hub.last_seen).getTime()) < 120_000;
	}

	$effect(() => { load(); });
</script>

<div class="mx-auto max-w-6xl space-y-6">
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-2xl font-bold text-surface-100">Hubs</h1>
			<p class="mt-1 text-sm text-surface-500">Physical hosts orchestrating multiple R3 clients</p>
		</div>
		<button onclick={refresh} class="btn-secondary flex items-center gap-2" disabled={refreshing}>
			<RefreshCw class="h-4 w-4 {refreshing ? 'animate-spin' : ''}" />
			Refresh
		</button>
	</div>

	{#if error}
		<div class="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">{error}</div>
	{/if}

	{#if loading}
		<div class="flex justify-center py-12">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
		</div>
	{:else if hubs.length === 0}
		<div class="rounded-xl border border-surface-800 bg-surface-900 p-12 text-center">
			<HardDrive class="mx-auto h-12 w-12 text-surface-600" />
			<h2 class="mt-4 text-lg font-semibold text-surface-300">No hubs connected</h2>
			<p class="mt-2 text-sm text-surface-500">
				Install R3 in <span class="text-surface-300">hub</span> mode on a host and pair it via the
				<a href="/pairing" class="text-accent hover:underline">Pairing</a> page.
			</p>
		</div>
	{:else}
		<div class="grid gap-4">
			{#each hubs as hub (hub.id)}
				{@const online = isOnline(hub)}
				<div class="rounded-xl border border-surface-800 bg-surface-900 p-5">
					<div class="flex items-start justify-between gap-4">
						<div class="flex items-start gap-3">
							<div class="rounded-lg bg-surface-800 p-2">
								<HardDrive class="h-5 w-5 text-accent" />
							</div>
							<div>
								<a href={`/hubs/${hub.id}`} class="text-lg font-semibold text-surface-100 hover:text-accent">
									{hub.name || `hub-${hub.id}`}
								</a>
								<div class="mt-1 flex items-center gap-3 text-xs text-surface-500">
									<span class="flex items-center gap-1">
										{#if online}
											<Wifi class="h-3 w-3 text-green-400" /><span class="text-green-400">online</span>
										{:else}
											<WifiOff class="h-3 w-3 text-surface-600" /><span>offline</span>
										{/if}
									</span>
									<span>last seen {fmtAge(hub.last_seen)}</span>
									{#if hub.version}<span>v{hub.version}</span>{/if}
								</div>
							</div>
						</div>
						<div class="flex items-center gap-2">
							<a href={`/hubs/${hub.id}`} class="btn-secondary text-sm">Manage</a>
							<button onclick={() => deleteTarget = hub} class="btn-ghost text-red-400 hover:bg-red-500/10" title="Remove hub">
								<Trash2 class="h-4 w-4" />
							</button>
						</div>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

{#if deleteTarget}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4" role="dialog">
		<div class="w-full max-w-md rounded-xl border border-surface-800 bg-surface-900 p-6">
			<h2 class="text-lg font-semibold text-surface-100">Remove hub?</h2>
			<p class="mt-2 text-sm text-surface-400">
				This removes <span class="text-surface-100">{deleteTarget.name}</span> from the master <strong class="text-red-300">and fully uninstalls R3 from the host</strong> — every client this hub manages will also be uninstalled and its game server files removed. This cannot be undone.
			</p>
			<div class="mt-4 flex justify-end gap-2">
				<button onclick={() => deleteTarget = null} class="btn-secondary" disabled={deleting}>Cancel</button>
				<button onclick={confirmDelete} class="btn-danger" disabled={deleting}>
					{deleting ? 'Removing…' : 'Remove'}
				</button>
			</div>
		</div>
	</div>
{/if}
