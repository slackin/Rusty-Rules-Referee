<script>
	import { page } from '$app/stores';
	import LiveGearEditor from '$lib/livegear/LiveGearEditor.svelte';

	let serverId = $derived(Number($page.params.id));
</script>

<svelte:head>
	<title>Live Gear — Server #{serverId}</title>
</svelte:head>

<div class="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4">
	<LiveGearEditor {serverId} />
</div>
