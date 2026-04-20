<script>
	import { api } from '$lib/api.svelte.js';
	import { page } from '$app/stores';
	import { Server, Wifi, WifiOff, Users, Map, Terminal, MessageSquare, ArrowLeft, UserX, ShieldBan, Send, RefreshCw, Settings, Save } from 'lucide-svelte';

	let serverId = $derived(Number($page.params.id));
	let server = $state(null);
	let loading = $state(true);
	let error = $state('');

	// Server config
	let configLoading = $state(false);
	let configSaving = $state(false);
	let configResult = $state(null);
	let configAddress = $state('');
	let configPort = $state(27960);
	let configRconPassword = $state('');
	let configGameLog = $state('');

	// RCON
	let rconCommand = $state('');
	let rconHistory = $state([]);
	let rconSending = $state(false);

	// Say
	let sayMessage = $state('');
	let saySending = $state(false);
	let sayResult = $state(null);

	// Kick/Ban
	let actionType = $state(null); // 'kick' | 'ban'
	let actionCid = $state('');
	let actionReason = $state('');
	let actionDuration = $state(60);
	let actionSending = $state(false);
	let actionResult = $state(null);

	async function loadServer() {
		try {
			server = await api.server(serverId);
			error = '';
		} catch (e) {
			error = e.message || 'Failed to load server';
		}
		loading = false;
	}

	async function loadConfig() {
		configLoading = true;
		try {
			const res = await api.serverConfig(serverId);
			if (res.config) {
				configAddress = res.config.address || '';
				configPort = res.config.port || 27960;
				configRconPassword = res.config.rcon_password || '';
				configGameLog = res.config.game_log || '';
			}
		} catch (e) {
			// Config may not exist yet — that's fine
		}
		configLoading = false;
	}

	async function saveConfig() {
		configSaving = true;
		configResult = null;
		try {
			const payload = {
				address: configAddress,
				port: Number(configPort),
				rcon_password: configRconPassword,
			};
			if (configGameLog.trim()) payload.game_log = configGameLog;
			const res = await api.updateServerConfig(serverId, payload);
			configResult = { ok: true, message: res.message || 'Configuration saved and pushed' };
			// Reload server info to reflect updated address/port
			loadServer();
		} catch (e) {
			configResult = { ok: false, message: e.message || 'Failed to save' };
		}
		configSaving = false;
	}

	function isUnconfigured() {
		return server && (!server.address || server.address === '0.0.0.0' || server.port === 0);
	}

	async function sendRcon() {
		if (!rconCommand.trim()) return;
		const cmd = rconCommand;
		rconCommand = '';
		rconSending = true;
		rconHistory = [...rconHistory, { type: 'cmd', text: cmd }];
		try {
			const res = await api.serverRcon(serverId, cmd);
			rconHistory = [...rconHistory, { type: 'ok', text: res.message }];
		} catch (e) {
			rconHistory = [...rconHistory, { type: 'err', text: e.message || 'Failed' }];
		}
		rconSending = false;
	}

	async function sendSay() {
		if (!sayMessage.trim()) return;
		saySending = true;
		sayResult = null;
		try {
			await api.serverSay(serverId, sayMessage);
			sayResult = { ok: true, message: 'Message sent' };
			sayMessage = '';
		} catch (e) {
			sayResult = { ok: false, message: e.message || 'Failed' };
		}
		saySending = false;
	}

	async function doAction() {
		if (!actionCid.trim()) return;
		actionSending = true;
		actionResult = null;
		try {
			if (actionType === 'kick') {
				await api.serverKick(serverId, actionCid, actionReason || undefined);
			} else {
				await api.serverBan(serverId, actionCid, actionReason || undefined, actionDuration);
			}
			actionResult = { ok: true, message: `${actionType === 'kick' ? 'Kick' : 'Ban'} sent` };
			actionCid = '';
			actionReason = '';
		} catch (e) {
			actionResult = { ok: false, message: e.message || 'Failed' };
		}
		actionSending = false;
	}

	$effect(() => { loadServer(); loadConfig(); });
</script>

<div class="mx-auto max-w-5xl space-y-6">
	<!-- Back link -->
	<a href="/servers" class="inline-flex items-center gap-2 text-sm text-surface-500 hover:text-surface-300 transition-colors">
		<ArrowLeft class="h-4 w-4" />
		Back to Servers
	</a>

	{#if loading}
		<div class="flex justify-center py-12">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
		</div>
	{:else if error}
		<div class="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">{error}</div>
	{:else if server}
		<!-- Server header -->
		<div class="rounded-xl border border-surface-800 bg-surface-900 p-6">
			<div class="flex items-center gap-4">
				<div class="flex h-12 w-12 items-center justify-center rounded-lg {server.online ? 'bg-emerald-500/10' : 'bg-surface-800'}">
					{#if server.online}
						<Wifi class="h-6 w-6 text-emerald-400" />
					{:else}
						<WifiOff class="h-6 w-6 text-surface-500" />
					{/if}
				</div>
				<div class="flex-1">
					<h1 class="text-xl font-bold text-surface-100">{server.name}</h1>
					<div class="mt-1 flex items-center gap-4 text-sm text-surface-500">
						<span>{server.address}:{server.port}</span>
						<span class="rounded-full px-2 py-0.5 text-xs font-medium {server.online ? 'bg-emerald-500/10 text-emerald-400' : 'bg-surface-800 text-surface-500'}">
							{server.online ? 'Online' : 'Offline'}
						</span>
					</div>
				</div>
				<button onclick={() => { loading = true; loadServer(); }} class="btn-secondary flex items-center gap-2">
					<RefreshCw class="h-4 w-4" />
					Refresh
				</button>
			</div>

			{#if server.online}
				<div class="mt-5 grid grid-cols-2 gap-4 sm:grid-cols-4">
					<div class="rounded-lg bg-surface-800/50 p-3">
						<div class="text-xs text-surface-500">Players</div>
						<div class="mt-1 text-lg font-semibold text-surface-100">{server.player_count} <span class="text-sm text-surface-500">/ {server.max_clients}</span></div>
					</div>
					<div class="rounded-lg bg-surface-800/50 p-3">
						<div class="text-xs text-surface-500">Map</div>
						<div class="mt-1 text-lg font-semibold text-surface-100">{server.current_map || '—'}</div>
					</div>
					<div class="rounded-lg bg-surface-800/50 p-3">
						<div class="text-xs text-surface-500">Status</div>
						<div class="mt-1 text-lg font-semibold text-surface-100">{server.status}</div>
					</div>
					<div class="rounded-lg bg-surface-800/50 p-3">
						<div class="text-xs text-surface-500">Last Seen</div>
						<div class="mt-1 text-sm font-medium text-surface-200">{server.last_seen ? new Date(server.last_seen).toLocaleString() : '—'}</div>
					</div>
				</div>
			{/if}
		</div>

		{#if isUnconfigured()}
			<!-- Unconfigured banner -->
			<div class="rounded-xl border border-amber-500/30 bg-amber-500/5 p-6">
				<h2 class="mb-2 flex items-center gap-2 text-base font-semibold text-amber-400">
					<Settings class="h-4 w-4" />
					Game Server Configuration Required
				</h2>
				<p class="text-sm text-surface-400">This client has connected but the game server details haven't been configured yet. Fill in the details below to push the configuration to the client.</p>
			</div>
		{/if}

		<!-- Server Configuration -->
		<div class="rounded-xl border border-surface-800 bg-surface-900 p-6">
			<h2 class="mb-4 flex items-center gap-2 text-base font-semibold text-surface-100">
				<Settings class="h-4 w-4 text-surface-400" />
				Game Server Settings
			</h2>

			{#if configResult}
				<div class="mb-4 rounded-lg px-3 py-2 text-sm {configResult.ok ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}">
					{configResult.message}
				</div>
			{/if}

			<div class="grid gap-4 sm:grid-cols-2">
				<div>
					<label for="cfg-address" class="mb-1 block text-xs font-medium text-surface-400">Server IP Address</label>
					<input id="cfg-address" type="text" bind:value={configAddress} placeholder="e.g. 203.0.113.10" class="input text-sm" />
				</div>
				<div>
					<label for="cfg-port" class="mb-1 block text-xs font-medium text-surface-400">Game Port</label>
					<input id="cfg-port" type="number" bind:value={configPort} placeholder="27960" class="input text-sm" />
				</div>
				<div>
					<label for="cfg-rcon" class="mb-1 block text-xs font-medium text-surface-400">RCON Password</label>
					<input id="cfg-rcon" type="password" bind:value={configRconPassword} placeholder="RCON password" class="input text-sm" />
				</div>
				<div>
					<label for="cfg-log" class="mb-1 block text-xs font-medium text-surface-400">Game Log Path <span class="text-surface-600">(optional)</span></label>
					<input id="cfg-log" type="text" bind:value={configGameLog} placeholder="/path/to/games.log" class="input text-sm" />
				</div>
			</div>

			<div class="mt-4 flex items-center gap-3">
				<button onclick={saveConfig} class="btn-primary flex items-center gap-2" disabled={configSaving || !configAddress.trim() || !configRconPassword.trim()}>
					<Save class="h-4 w-4" />
					{configSaving ? 'Saving...' : 'Save & Push to Client'}
				</button>
				{#if configSaving}
					<div class="h-4 w-4 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
				{/if}
			</div>
		</div>

		{#if server.online}
			<!-- RCON Console -->
			<div class="rounded-xl border border-surface-800 bg-surface-900 p-6">
				<h2 class="mb-4 flex items-center gap-2 text-base font-semibold text-surface-100">
					<Terminal class="h-4 w-4 text-accent" />
					Remote Console
				</h2>

				{#if rconHistory.length > 0}
					<div class="mb-3 max-h-64 overflow-y-auto rounded-lg bg-surface-950 p-3 font-mono text-xs">
						{#each rconHistory as entry}
							{#if entry.type === 'cmd'}
								<div class="text-accent">&gt; {entry.text}</div>
							{:else if entry.type === 'ok'}
								<div class="text-surface-300">{entry.text}</div>
							{:else}
								<div class="text-red-400">{entry.text}</div>
							{/if}
						{/each}
					</div>
				{/if}

				<div class="flex gap-2">
					<input type="text" bind:value={rconCommand} placeholder="Enter RCON command..." class="input flex-1 font-mono text-sm"
						onkeydown={(e) => { if (e.key === 'Enter') sendRcon(); }} />
					<button onclick={sendRcon} class="btn-primary" disabled={rconSending || !rconCommand.trim()}>
						<Send class="h-4 w-4" />
					</button>
				</div>
			</div>

			<!-- Say & Actions -->
			<div class="grid gap-4 md:grid-cols-2">
				<!-- Broadcast -->
				<div class="rounded-xl border border-surface-800 bg-surface-900 p-6">
					<h2 class="mb-4 flex items-center gap-2 text-base font-semibold text-surface-100">
						<MessageSquare class="h-4 w-4 text-blue-400" />
						Broadcast Message
					</h2>

					{#if sayResult}
						<div class="mb-3 rounded-lg px-3 py-2 text-xs {sayResult.ok ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}">
							{sayResult.message}
						</div>
					{/if}

					<div class="flex gap-2">
						<input type="text" bind:value={sayMessage} placeholder="Message..." class="input flex-1 text-sm"
							onkeydown={(e) => { if (e.key === 'Enter') sendSay(); }} />
						<button onclick={sendSay} class="btn-primary" disabled={saySending || !sayMessage.trim()}>Send</button>
					</div>
				</div>

				<!-- Kick/Ban -->
				<div class="rounded-xl border border-surface-800 bg-surface-900 p-6">
					<h2 class="mb-4 flex items-center gap-2 text-base font-semibold text-surface-100">
						<ShieldBan class="h-4 w-4 text-red-400" />
						Kick / Ban Player
					</h2>

					{#if actionResult}
						<div class="mb-3 rounded-lg px-3 py-2 text-xs {actionResult.ok ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}">
							{actionResult.message}
						</div>
					{/if}

					<div class="space-y-3">
						<input type="text" bind:value={actionCid} placeholder="Player slot ID" class="input text-sm" />
						<input type="text" bind:value={actionReason} placeholder="Reason (optional)" class="input text-sm" />
						<div class="flex gap-2">
							<button onclick={() => { actionType = 'kick'; doAction(); }} class="btn-secondary flex-1 text-sm" disabled={actionSending || !actionCid.trim()}>
								<UserX class="mr-1 inline h-3.5 w-3.5" /> Kick
							</button>
							<button onclick={() => { actionType = 'ban'; doAction(); }} class="flex-1 rounded-lg bg-red-600 px-4 py-2 text-sm font-medium text-white hover:bg-red-700 transition-colors disabled:opacity-50" disabled={actionSending || !actionCid.trim()}>
								<ShieldBan class="mr-1 inline h-3.5 w-3.5" /> Ban
							</button>
						</div>
					</div>
				</div>
			</div>
		{/if}
	{/if}
</div>

<style>
	.input { @apply w-full rounded-lg border border-surface-700 bg-surface-800 px-3 py-2 text-surface-100 placeholder-surface-600 focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent; }
	.btn-primary { @apply rounded-lg bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent/90 transition-colors disabled:opacity-50; }
	.btn-secondary { @apply rounded-lg border border-surface-700 bg-surface-800 px-4 py-2 text-sm font-medium text-surface-300 hover:bg-surface-700 transition-colors disabled:opacity-50; }
</style>
