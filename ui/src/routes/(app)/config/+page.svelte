<script>
	import { onMount } from 'svelte';
	import { api } from '$lib/api.svelte.js';
	import { Save, RotateCcw } from 'lucide-svelte';

	let config = $state(null);
	let original = $state('');
	let configText = $state('');
	let loading = $state(true);
	let saving = $state(false);
	let message = $state('');
	let messageType = $state('');

	onMount(async () => {
		try {
			config = await api.getConfig();
			configText = JSON.stringify(config, null, 2);
			original = configText;
		} catch (e) {
			message = e.message;
			messageType = 'error';
		}
		loading = false;
	});

	async function save() {
		saving = true;
		message = '';
		try {
			const parsed = JSON.parse(configText);
			await api.updateConfig(parsed);
			message = 'Configuration saved successfully. Restart may be needed for some changes.';
			messageType = 'success';
			original = configText;
		} catch (e) {
			message = e.message;
			messageType = 'error';
		}
		saving = false;
	}

	function reset() {
		configText = original;
		message = '';
	}

	let isDirty = $derived(configText !== original);
</script>

<div class="space-y-6 animate-fade-in">
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-2xl font-semibold">Configuration</h1>
			<p class="mt-1 text-sm text-surface-500">Edit the bot's TOML configuration as JSON</p>
		</div>
		<div class="flex gap-2">
			<button class="btn-secondary btn-sm" onclick={reset} disabled={!isDirty}>
				<RotateCcw class="h-3.5 w-3.5" /> Reset
			</button>
			<button class="btn-primary btn-sm" onclick={save} disabled={saving || !isDirty}>
				<Save class="h-3.5 w-3.5" /> Save
			</button>
		</div>
	</div>

	{#if message}
		<div class="rounded-lg px-4 py-3 text-sm {messageType === 'error' ? 'bg-red-500/10 text-red-400 ring-1 ring-red-500/20' : 'bg-emerald-500/10 text-emerald-400 ring-1 ring-emerald-500/20'}">
			{message}
		</div>
	{/if}

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
		</div>
	{:else}
		<div class="card overflow-hidden">
			<textarea
				bind:value={configText}
				class="w-full bg-transparent p-5 font-mono text-sm text-surface-200 focus:outline-none resize-none"
				rows="30"
				spellcheck="false"
			></textarea>
		</div>
	{/if}
</div>
