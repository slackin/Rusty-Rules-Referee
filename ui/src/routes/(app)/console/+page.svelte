<script>
	import { onMount } from 'svelte';
	import { api } from '$lib/api.js';
	import { Send, Terminal } from 'lucide-svelte';

	let command = $state('');
	let history = $state([]);
	let loading = $state(false);

	async function execute() {
		if (!command.trim()) return;
		const cmd = command.trim();
		command = '';
		loading = true;
		history = [...history, { type: 'cmd', text: cmd, time: new Date() }];

		try {
			const res = await api.rcon(cmd);
			history = [...history, { type: 'res', text: res.response ?? res, time: new Date() }];
		} catch (e) {
			history = [...history, { type: 'err', text: e.message, time: new Date() }];
		}
		loading = false;

		// Scroll to bottom
		setTimeout(() => {
			const el = document.getElementById('console-output');
			if (el) el.scrollTop = el.scrollHeight;
		}, 50);
	}

	async function say() {
		const msg = command.trim();
		if (!msg) return;
		command = '';
		try {
			await api.say(msg);
			history = [...history, { type: 'cmd', text: `[SAY] ${msg}`, time: new Date() }];
		} catch (e) {
			history = [...history, { type: 'err', text: e.message, time: new Date() }];
		}
	}

	function handleKey(e) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			execute();
		}
	}
</script>

<div class="flex h-[calc(100vh-8rem)] flex-col space-y-4 animate-fade-in">
	<div>
		<h1 class="text-2xl font-semibold">Console</h1>
		<p class="mt-1 text-sm text-surface-500">Execute RCON commands on the server</p>
	</div>

	<!-- Output -->
	<div id="console-output" class="card flex-1 overflow-y-auto p-4 font-mono text-sm">
		{#if history.length === 0}
			<div class="flex h-full items-center justify-center text-surface-600">
				<div class="text-center">
					<Terminal class="mx-auto mb-2 h-8 w-8" />
					<p>Type a command below to get started</p>
				</div>
			</div>
		{:else}
			<div class="space-y-2">
				{#each history as entry}
					{#if entry.type === 'cmd'}
						<div class="flex gap-2">
							<span class="text-accent">❯</span>
							<span class="text-surface-200">{entry.text}</span>
						</div>
					{:else if entry.type === 'res'}
						<pre class="whitespace-pre-wrap text-surface-400 pl-5">{entry.text}</pre>
					{:else}
						<div class="pl-5 text-red-400">{entry.text}</div>
					{/if}
				{/each}
			</div>
		{/if}
	</div>

	<!-- Input -->
	<div class="card p-3">
		<div class="flex gap-2">
			<div class="relative flex-1">
				<span class="absolute left-3 top-1/2 -translate-y-1/2 text-accent font-mono text-sm">❯</span>
				<input
					type="text"
					bind:value={command}
					onkeydown={handleKey}
					class="input pl-8 font-mono"
					placeholder="Enter RCON command…"
					disabled={loading}
				/>
			</div>
			<button class="btn-primary" onclick={execute} disabled={loading || !command.trim()} title="Execute">
				<Send class="h-4 w-4" />
			</button>
			<button class="btn-secondary" onclick={say} disabled={!command.trim()} title="Say to server">
				Say
			</button>
		</div>
	</div>
</div>
