<script>
	import { api } from '$lib/api.svelte.js';
	import { Link, Unlink, Copy, Check, Clock, Terminal } from 'lucide-svelte';

	let enabled = $state(false);
	let token = $state('');
	let expiresAt = $state('');
	let connectCommand = $state('');
	let loading = $state(false);
	let error = $state('');
	let expiryMinutes = $state(30);
	let copied = $state('');
	let disabling = $state(false);

	async function enablePairing() {
		loading = true;
		error = '';
		try {
			const res = await api.enablePairing(expiryMinutes);
			token = res.token;
			expiresAt = res.expires_at;
			connectCommand = res.connect_command;
			enabled = true;
		} catch (e) {
			error = e.message || 'Failed to enable pairing';
		}
		loading = false;
	}

	async function disablePairing() {
		disabling = true;
		error = '';
		try {
			await api.disablePairing();
			enabled = false;
			token = '';
			expiresAt = '';
			connectCommand = '';
		} catch (e) {
			error = e.message || 'Failed to disable pairing';
		}
		disabling = false;
	}

	async function copyToClipboard(text, label) {
		try {
			await navigator.clipboard.writeText(text);
			copied = label;
			setTimeout(() => { copied = ''; }, 2000);
		} catch {
			// fallback
		}
	}

	function formatExpiry(iso) {
		if (!iso) return '';
		const d = new Date(iso);
		return d.toLocaleString();
	}
</script>

<div class="mx-auto max-w-3xl space-y-6">
	<div>
		<h1 class="text-2xl font-bold text-surface-100">Server Pairing</h1>
		<p class="mt-1 text-sm text-surface-500">Generate a quick-connect token so game server bots can pair with this master</p>
	</div>

	{#if error}
		<div class="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">{error}</div>
	{/if}

	{#if !enabled}
		<!-- Enable pairing -->
		<div class="rounded-xl border border-surface-800 bg-surface-900 p-8 text-center">
			<div class="mx-auto flex h-16 w-16 items-center justify-center rounded-2xl bg-accent/10">
				<Link class="h-8 w-8 text-accent" />
			</div>
			<h2 class="mt-5 text-lg font-semibold text-surface-100">Quick-Connect Pairing</h2>
			<p class="mt-2 text-sm text-surface-400 max-w-md mx-auto">
				Generate a time-limited token that allows game server bots to register with this master. The token will automatically expire after the specified duration.
			</p>

			<div class="mt-6 flex items-center justify-center gap-4">
				<div class="flex items-center gap-2">
					<Clock class="h-4 w-4 text-surface-500" />
					<label for="expiry" class="text-sm text-surface-400">Token valid for</label>
					<select id="expiry" bind:value={expiryMinutes} class="rounded-lg border border-surface-700 bg-surface-800 px-3 py-1.5 text-sm text-surface-100">
						<option value={15}>15 minutes</option>
						<option value={30}>30 minutes</option>
						<option value={60}>1 hour</option>
						<option value={120}>2 hours</option>
						<option value={1440}>24 hours</option>
					</select>
				</div>
			</div>

			<button onclick={enablePairing} class="mt-6 rounded-lg bg-accent px-6 py-2.5 text-sm font-medium text-white hover:bg-accent/90 transition-colors disabled:opacity-50" disabled={loading}>
				{loading ? 'Generating...' : 'Enable Pairing'}
			</button>
		</div>
	{:else}
		<!-- Active pairing -->
		<div class="rounded-xl border border-accent/30 bg-accent/5 p-6">
			<div class="flex items-center gap-3">
				<div class="flex h-10 w-10 items-center justify-center rounded-lg bg-accent/10">
					<Link class="h-5 w-5 text-accent" />
				</div>
				<div class="flex-1">
					<div class="text-base font-semibold text-surface-100">Pairing Active</div>
					<div class="text-sm text-surface-500">Expires: {formatExpiry(expiresAt)}</div>
				</div>
				<button onclick={disablePairing} class="rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-2 text-sm font-medium text-red-400 hover:bg-red-500/20 transition-colors disabled:opacity-50" disabled={disabling}>
					<Unlink class="mr-1.5 inline h-4 w-4" />
					{disabling ? 'Disabling...' : 'Disable'}
				</button>
			</div>
		</div>

		<!-- Token -->
		<div class="rounded-xl border border-surface-800 bg-surface-900 p-6">
			<h3 class="mb-4 text-base font-semibold text-surface-100">Pairing Token</h3>
			<div class="flex items-center gap-2 rounded-lg bg-surface-950 p-3">
				<code class="flex-1 select-all font-mono text-sm text-amber-400 break-all">{token}</code>
				<button onclick={() => copyToClipboard(token, 'token')} class="rounded-lg p-2 text-surface-400 hover:bg-surface-800 hover:text-surface-200 transition-colors" title="Copy token">
					{#if copied === 'token'}
						<Check class="h-4 w-4 text-emerald-400" />
					{:else}
						<Copy class="h-4 w-4" />
					{/if}
				</button>
			</div>
		</div>

		<!-- Connect command -->
		{#if connectCommand}
			<div class="rounded-xl border border-surface-800 bg-surface-900 p-6">
				<h3 class="mb-2 text-base font-semibold text-surface-100 flex items-center gap-2">
					<Terminal class="h-4 w-4 text-surface-400" />
					Connect Command
				</h3>
				<p class="mb-3 text-sm text-surface-500">Run this on the game server bot to pair it:</p>
				<div class="flex items-center gap-2 rounded-lg bg-surface-950 p-3">
					<code class="flex-1 select-all font-mono text-xs text-surface-200 break-all">{connectCommand}</code>
					<button onclick={() => copyToClipboard(connectCommand, 'cmd')} class="rounded-lg p-2 text-surface-400 hover:bg-surface-800 hover:text-surface-200 transition-colors" title="Copy command">
						{#if copied === 'cmd'}
							<Check class="h-4 w-4 text-emerald-400" />
						{:else}
							<Copy class="h-4 w-4" />
						{/if}
					</button>
				</div>
			</div>
		{/if}
	{/if}
</div>
