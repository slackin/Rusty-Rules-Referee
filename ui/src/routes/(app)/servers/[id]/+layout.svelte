<script>
	import { page } from '$app/stores';
	import { api } from '$lib/api.svelte.js';
	import { LayoutDashboard, Users, ShieldBan, MessageSquare, ScrollText, Puzzle, Map, ListOrdered, FileCode } from 'lucide-svelte';

	let serverId = $derived(Number($page.params.id));
	let server = $state(null);

	// Light fetch just for header display. The individual pages do their own
	// heavy lifting.
	$effect(() => {
		(async () => {
			try {
				server = await api.server(serverId);
			} catch (_) {}
		})();
	});

	const tabs = [
		{ href: '', label: 'Dashboard', icon: LayoutDashboard },
		{ href: 'players', label: 'Players', icon: Users },
		{ href: 'penalties', label: 'Penalties', icon: ShieldBan },
		{ href: 'chat', label: 'Chat', icon: MessageSquare },
		{ href: 'audit-log', label: 'Audit Log', icon: ScrollText },
		{ href: 'plugins', label: 'Plugins', icon: Puzzle },
		{ href: 'map-config', label: 'Map Config', icon: Map },
		{ href: 'mapcycle', label: 'Mapcycle', icon: ListOrdered },
		{ href: 'server-cfg', label: 'server.cfg', icon: FileCode },
	];

	let currentTab = $derived.by(() => {
		const m = $page.url.pathname.match(new RegExp(`/servers/${serverId}/?([^/]*)`));
		return m ? m[1] : '';
	});
</script>

<div class="max-w-7xl mx-auto px-4 py-6">
	{#if server}
		<div class="mb-4">
			<h1 class="text-2xl font-bold">{server.name || `Server #${serverId}`}</h1>
			<p class="text-sm text-gray-500">
				{server.address}:{server.port} · {server.online ? 'online' : 'offline'}
				{#if server.current_map}· {server.current_map}{/if}
				{#if server.player_count != null}· {server.player_count}/{server.max_clients}{/if}
			</p>
		</div>
	{/if}

	<nav class="flex flex-wrap gap-1 border-b mb-4">
		{#each tabs as t}
			{@const active = t.href === currentTab}
			<a
				href={`/servers/${serverId}/${t.href}`}
				class="px-3 py-2 text-sm flex items-center gap-1 border-b-2 {active ? 'border-blue-500 text-blue-600 font-semibold' : 'border-transparent text-gray-600 hover:text-gray-900'}"
			>
				<svelte:component this={t.icon} size={16} />
				{t.label}
			</a>
		{/each}
	</nav>

	<slot />
</div>
