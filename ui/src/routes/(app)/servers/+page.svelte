<script>
	import { api } from '$lib/api.svelte.js';
	import { Server, Wifi, WifiOff, Users, Map, Trash2, Terminal, MessageSquare, RefreshCw, AlertTriangle } from 'lucide-svelte';

	let servers = $state([]);
	let loading = $state(true);
	let error = $state('');
	let refreshing = $state(false);

	// RCON modal
	let rconServer = $state(null);
	let rconCommand = $state('');
	let rconSending = $state(false);
	let rconResult = $state(null);

	// Say modal
	let sayServer = $state(null);
	let sayMessage = $state('');
	let saySending = $state(false);
	let sayResult = $state(null);

	// Delete confirm
	let deleteTarget = $state(null);
	let deleting = $state(false);

	async function loadServers() {
		try {
			servers = await api.servers();
			error = '';
		} catch (e) {
			error = e.message || 'Failed to load servers';
		}
		loading = false;
	}

	async function refresh() {
		refreshing = true;
		await loadServers();
		refreshing = false;
	}

	async function sendRcon() {
		if (!rconCommand.trim() || !rconServer) return;
		rconSending = true;
		rconResult = null;
		try {
			const res = await api.serverRcon(rconServer.id, rconCommand);
			rconResult = { ok: true, message: res.message };
			rconCommand = '';
		} catch (e) {
			rconResult = { ok: false, message: e.message || 'Failed' };
		}
		rconSending = false;
	}

	async function sendSay() {
		if (!sayMessage.trim() || !sayServer) return;
		saySending = true;
		sayResult = null;
		try {
			const res = await api.serverSay(sayServer.id, sayMessage);
			sayResult = { ok: true, message: res.message };
			sayMessage = '';
		} catch (e) {
			sayResult = { ok: false, message: e.message || 'Failed' };
		}
		saySending = false;
	}

	async function confirmDelete() {
		if (!deleteTarget) return;
		deleting = true;
		try {
			await api.deleteServer(deleteTarget.id);
			servers = servers.filter(s => s.id !== deleteTarget.id);
			deleteTarget = null;
		} catch (e) {
			error = e.message || 'Failed to delete server';
		}
		deleting = false;
	}

	$effect(() => { loadServers(); });

	function isUnconfigured(server) {
		return !server.address || server.address === '0.0.0.0' || server.port === 0;
	}
</script>

<div class="mx-auto max-w-6xl space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-2xl font-bold text-surface-100">Connected Servers</h1>
			<p class="mt-1 text-sm text-surface-500">Manage game servers connected to this master</p>
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
	{:else if servers.length === 0}
		<div class="rounded-xl border border-surface-800 bg-surface-900 p-12 text-center">
			<Server class="mx-auto h-12 w-12 text-surface-600" />
			<h2 class="mt-4 text-lg font-semibold text-surface-300">No servers connected</h2>
			<p class="mt-2 text-sm text-surface-500">Use the <a href="/pairing" class="text-accent hover:underline">Pairing</a> page to connect game server bots to this master.</p>
		</div>
	{:else}
		<!-- Server cards -->
		<div class="grid gap-4">
			{#each servers as server (server.id)}
				<div class="rounded-xl border border-surface-800 bg-surface-900 p-5 transition-colors hover:border-surface-700">
					<div class="flex items-start justify-between">
						<div class="flex items-center gap-4">
							<!-- Status indicator -->
							<div class="flex h-10 w-10 items-center justify-center rounded-lg {server.online ? 'bg-emerald-500/10' : 'bg-surface-800'}">
								{#if server.online}
									<Wifi class="h-5 w-5 text-emerald-400" />
								{:else}
									<WifiOff class="h-5 w-5 text-surface-500" />
								{/if}
							</div>
							<div>
								<div class="flex items-center gap-2">
									<a href="/servers/{server.id}" class="text-base font-semibold text-surface-100 hover:text-accent transition-colors">
										{server.name}
									</a>
									{#if isUnconfigured(server)}
										<span class="rounded-full px-2 py-0.5 text-xs font-medium bg-amber-500/10 text-amber-400 flex items-center gap-1">
											<AlertTriangle class="h-3 w-3" />
											Needs Setup
										</span>
									{:else}
										<span class="rounded-full px-2 py-0.5 text-xs font-medium {server.online ? 'bg-emerald-500/10 text-emerald-400' : 'bg-surface-800 text-surface-500'}">
											{server.online ? 'Online' : 'Offline'}
										</span>
									{/if}
								</div>
								<div class="mt-1 text-sm text-surface-500">
									{#if isUnconfigured(server)}
										<a href="/servers/{server.id}" class="text-amber-400 hover:text-amber-300 transition-colors">Configure game server &rarr;</a>
									{:else}
										{server.address}:{server.port}
									{/if}
								</div>
							</div>
						</div>

						<!-- Actions -->
						<div class="flex items-center gap-2">
							{#if server.online}
								<button onclick={() => { rconServer = server; rconCommand = ''; rconResult = null; }} class="rounded-lg p-2 text-surface-400 hover:bg-surface-800 hover:text-surface-200" title="RCON">
									<Terminal class="h-4 w-4" />
								</button>
								<button onclick={() => { sayServer = server; sayMessage = ''; sayResult = null; }} class="rounded-lg p-2 text-surface-400 hover:bg-surface-800 hover:text-surface-200" title="Say">
									<MessageSquare class="h-4 w-4" />
								</button>
							{/if}
							<button onclick={() => { deleteTarget = server; }} class="rounded-lg p-2 text-surface-400 hover:bg-red-500/20 hover:text-red-400" title="Remove server">
								<Trash2 class="h-4 w-4" />
							</button>
						</div>
					</div>

					{#if server.online}
						<div class="mt-4 flex gap-6 text-sm">
							<div class="flex items-center gap-2 text-surface-400">
								<Users class="h-3.5 w-3.5" />
								<span class="text-surface-200">{server.player_count}</span>
								<span class="text-surface-600">/ {server.max_clients}</span>
							</div>
							{#if server.current_map}
								<div class="flex items-center gap-2 text-surface-400">
									<Map class="h-3.5 w-3.5" />
									<span class="text-surface-200">{server.current_map}</span>
								</div>
							{/if}
						</div>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>

<!-- RCON Modal -->
{#if rconServer}
	<div class="fixed inset-0 z-[100] flex items-center justify-center bg-black/60 backdrop-blur-sm">
		<div class="w-full max-w-lg rounded-xl border border-surface-700 bg-surface-900 p-6 shadow-2xl">
			<h3 class="mb-1 text-lg font-semibold text-surface-100">RCON — {rconServer.name}</h3>
			<p class="mb-4 text-sm text-surface-500">Send a remote console command to this server</p>

			{#if rconResult}
				<div class="mb-3 rounded-lg px-3 py-2 text-sm {rconResult.ok ? 'bg-emerald-500/10 border border-emerald-500/20 text-emerald-400' : 'bg-red-500/10 border border-red-500/20 text-red-400'}">
					{rconResult.message}
				</div>
			{/if}

			<div class="flex gap-2">
				<input type="text" bind:value={rconCommand} placeholder="e.g. status" class="input flex-1 font-mono text-sm"
					onkeydown={(e) => { if (e.key === 'Enter') sendRcon(); }} />
				<button onclick={sendRcon} class="btn-primary" disabled={rconSending || !rconCommand.trim()}>
					{rconSending ? 'Sending...' : 'Send'}
				</button>
			</div>

			<div class="mt-4 flex justify-end">
				<button onclick={() => { rconServer = null; }} class="btn-secondary text-sm">Close</button>
			</div>
		</div>
	</div>
{/if}

<!-- Say Modal -->
{#if sayServer}
	<div class="fixed inset-0 z-[100] flex items-center justify-center bg-black/60 backdrop-blur-sm">
		<div class="w-full max-w-lg rounded-xl border border-surface-700 bg-surface-900 p-6 shadow-2xl">
			<h3 class="mb-1 text-lg font-semibold text-surface-100">Broadcast — {sayServer.name}</h3>
			<p class="mb-4 text-sm text-surface-500">Send a message to all players on this server</p>

			{#if sayResult}
				<div class="mb-3 rounded-lg px-3 py-2 text-sm {sayResult.ok ? 'bg-emerald-500/10 border border-emerald-500/20 text-emerald-400' : 'bg-red-500/10 border border-red-500/20 text-red-400'}">
					{sayResult.message}
				</div>
			{/if}

			<div class="flex gap-2">
				<input type="text" bind:value={sayMessage} placeholder="Message to broadcast..." class="input flex-1 text-sm"
					onkeydown={(e) => { if (e.key === 'Enter') sendSay(); }} />
				<button onclick={sendSay} class="btn-primary" disabled={saySending || !sayMessage.trim()}>
					{saySending ? 'Sending...' : 'Send'}
				</button>
			</div>

			<div class="mt-4 flex justify-end">
				<button onclick={() => { sayServer = null; }} class="btn-secondary text-sm">Close</button>
			</div>
		</div>
	</div>
{/if}

<!-- Delete Confirm -->
{#if deleteTarget}
	<div class="fixed inset-0 z-[100] flex items-center justify-center bg-black/60 backdrop-blur-sm">
		<div class="w-full max-w-md rounded-xl border border-surface-700 bg-surface-900 p-6 shadow-2xl">
			<h3 class="mb-2 text-lg font-semibold text-surface-100">Remove Server</h3>
			<p class="mb-4 text-sm text-surface-400">
				Are you sure you want to remove <strong class="text-surface-200">{deleteTarget.name}</strong>? This will unregister it from the master. The server bot will need to re-pair to reconnect.
			</p>
			<div class="flex justify-end gap-3">
				<button onclick={() => { deleteTarget = null; }} class="btn-secondary" disabled={deleting}>Cancel</button>
				<button onclick={confirmDelete} class="rounded-lg bg-red-600 px-4 py-2 text-sm font-medium text-white hover:bg-red-700 transition-colors" disabled={deleting}>
					{deleting ? 'Removing...' : 'Remove'}
				</button>
			</div>
		</div>
	</div>
{/if}

<style>
	.input { @apply w-full rounded-lg border border-surface-700 bg-surface-800 px-3 py-2 text-surface-100 placeholder-surface-600 focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent; }
	.btn-primary { @apply rounded-lg bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent/90 transition-colors disabled:opacity-50; }
	.btn-secondary { @apply rounded-lg border border-surface-700 bg-surface-800 px-4 py-2 text-sm font-medium text-surface-300 hover:bg-surface-700 transition-colors disabled:opacity-50; }
</style>
