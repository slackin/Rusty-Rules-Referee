<script>
	import { onMount } from 'svelte';
	import { api } from '$lib/api.svelte.js';
	import { Search } from 'lucide-svelte';

	let commands = $state([]);
	let loading = $state(true);
	let error = $state('');
	let search = $state('');
	let filterPlugin = $state('all');
	let filterLevel = $state('all');

	onMount(async () => {
		try {
			commands = await api.commands();
		} catch (e) {
			error = 'Failed to load commands: ' + e.message;
		}
		loading = false;
	});

	let plugins = $derived([...new Set(commands.map(c => c.plugin))].sort());
	let levels = $derived([...new Set(commands.map(c => c.level))]);

	const levelOrder = ['Guest', 'User', 'Mod', 'Admin', 'Senior Admin', 'Super Admin'];
	let sortedLevels = $derived(levelOrder.filter(l => levels.includes(l)));

	let filtered = $derived(
		commands.filter(cmd => {
			if (filterPlugin !== 'all' && cmd.plugin !== filterPlugin) return false;
			if (filterLevel !== 'all' && cmd.level !== filterLevel) return false;
			if (search) {
				const q = search.toLowerCase();
				return cmd.name.toLowerCase().includes(q) ||
					cmd.description.toLowerCase().includes(q) ||
					cmd.syntax.toLowerCase().includes(q);
			}
			return true;
		})
	);

	let grouped = $derived(() => {
		const groups = {};
		for (const cmd of filtered) {
			if (!groups[cmd.plugin]) groups[cmd.plugin] = [];
			groups[cmd.plugin].push(cmd);
		}
		return Object.entries(groups).sort((a, b) => a[0].localeCompare(b[0]));
	});

	const levelColors = {
		'Guest': 'bg-zinc-600',
		'User': 'bg-blue-600',
		'Mod': 'bg-green-600',
		'Admin': 'bg-yellow-600',
		'Senior Admin': 'bg-orange-600',
		'Super Admin': 'bg-red-600',
	};
</script>

<svelte:head><title>Commands | R3</title></svelte:head>

<div class="p-6 max-w-5xl mx-auto">
	<h1 class="text-2xl font-bold text-white mb-6">Command Reference</h1>

	<!-- Filters -->
	<div class="flex flex-wrap gap-3 mb-6">
		<div class="relative flex-1 min-w-[200px]">
			<Search size={16} class="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500"/>
			<input type="text" bind:value={search} placeholder="Search commands..."
				class="w-full pl-9 pr-3 py-2 bg-zinc-800 border border-zinc-700 rounded-lg text-white placeholder-zinc-500 focus:outline-none focus:border-blue-500 text-sm"/>
		</div>
		<select bind:value={filterPlugin}
			class="px-3 py-2 bg-zinc-800 border border-zinc-700 rounded-lg text-white text-sm focus:outline-none focus:border-blue-500">
			<option value="all">All Plugins</option>
			{#each plugins as p}
				<option value={p}>{p}</option>
			{/each}
		</select>
		<select bind:value={filterLevel}
			class="px-3 py-2 bg-zinc-800 border border-zinc-700 rounded-lg text-white text-sm focus:outline-none focus:border-blue-500">
			<option value="all">All Levels</option>
			{#each sortedLevels as l}
				<option value={l}>{l}</option>
			{/each}
		</select>
	</div>

	{#if loading}
		<div class="text-zinc-400 text-center py-12">Loading commands...</div>
	{:else if error}
		<div class="p-3 bg-red-500/20 border border-red-500/40 rounded-lg text-red-300 text-sm">{error}</div>
	{:else}
		<div class="text-xs text-zinc-500 mb-4">{filtered.length} command{filtered.length !== 1 ? 's' : ''}</div>

		{#each grouped() as [plugin, cmds]}
			<div class="mb-6">
				<h2 class="text-sm font-semibold text-zinc-400 uppercase tracking-wider mb-2">{plugin}</h2>
				<div class="bg-zinc-800/40 border border-zinc-700/50 rounded-lg overflow-hidden">
					<table class="w-full text-sm">
						<thead>
							<tr class="border-b border-zinc-700/50">
								<th class="text-left px-4 py-2 text-zinc-500 font-medium">Command</th>
								<th class="text-left px-4 py-2 text-zinc-500 font-medium">Syntax</th>
								<th class="text-left px-4 py-2 text-zinc-500 font-medium">Description</th>
								<th class="text-left px-4 py-2 text-zinc-500 font-medium w-28">Level</th>
							</tr>
						</thead>
						<tbody>
							{#each cmds as cmd}
								<tr class="border-b border-zinc-700/30 hover:bg-zinc-700/20">
									<td class="px-4 py-2 text-white font-mono">!{cmd.name}</td>
									<td class="px-4 py-2 text-zinc-300 font-mono text-xs">{cmd.syntax}</td>
									<td class="px-4 py-2 text-zinc-400">{cmd.description}</td>
									<td class="px-4 py-2">
										<span class="inline-block px-2 py-0.5 rounded text-xs text-white {levelColors[cmd.level] || 'bg-zinc-600'}">
											{cmd.level}
										</span>
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</div>
		{/each}

		{#if filtered.length === 0}
			<div class="text-center py-12 text-zinc-500">No commands match your filters.</div>
		{/if}
	{/if}
</div>
