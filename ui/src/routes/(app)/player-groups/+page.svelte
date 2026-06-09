<script>
	import { api } from '$lib/api.svelte.js';
	import { goto } from '$app/navigation';
	import { UsersRound, Plus, Trash2, ChevronRight } from 'lucide-svelte';

	let groups = $state([]);
	let error = $state('');
	let loading = $state(true);
	let creating = $state(false);
	let newName = $state('');
	let newDesc = $state('');
	let createError = $state('');

	async function load() {
		loading = true;
		error = '';
		try {
			groups = await api.playerGroups();
		} catch (e) {
			error = e.message;
		} finally {
			loading = false;
		}
	}

	async function create() {
		createError = '';
		if (!newName.trim()) { createError = 'Name is required'; return; }
		creating = true;
		try {
			const r = await api.createPlayerGroup({ name: newName.trim(), description: newDesc.trim() });
			newName = '';
			newDesc = '';
			await load();
			goto(`/player-groups/${r.id}`);
		} catch (e) {
			createError = e.message;
		} finally {
			creating = false;
		}
	}

	async function remove(id, name) {
		if (!confirm(`Delete group "${name}" and all its members? This cannot be undone.`)) return;
		try {
			await api.deletePlayerGroup(id);
			await load();
		} catch (e) {
			error = e.message;
		}
	}

	$effect(() => { load(); });
</script>

<div class="max-w-4xl mx-auto px-4 py-6">
	<div class="flex items-center gap-2 mb-6">
		<UsersRound size={22} class="text-accent" />
		<h1 class="text-2xl font-bold">Player Groups</h1>
	</div>
	<p class="text-surface-400 text-sm mb-6">
		Named collections of player permission records shared across multiple game servers.
		Assign a group to a server and its members' permissions are merged with that server's local player list.
	</p>

	{#if error}<div class="text-red-400 mb-4 text-sm">{error}</div>{/if}

	<!-- Create new group -->
	<div class="card p-4 mb-6">
		<h2 class="text-sm font-semibold uppercase tracking-wide text-surface-500 mb-3">Create New Group</h2>
		<div class="flex flex-col sm:flex-row gap-3">
			<input
				bind:value={newName}
				placeholder="Group name (e.g. ATL Admins)"
				class="input flex-1"
				onkeydown={(e) => e.key === 'Enter' && create()}
			/>
			<input
				bind:value={newDesc}
				placeholder="Description (optional)"
				class="input flex-1"
				onkeydown={(e) => e.key === 'Enter' && create()}
			/>
			<button onclick={create} disabled={creating} class="btn-accent flex items-center gap-1 whitespace-nowrap">
				<Plus size={16} />
				{creating ? 'Creating…' : 'Create'}
			</button>
		</div>
		{#if createError}<p class="text-red-400 text-xs mt-2">{createError}</p>{/if}
	</div>

	<!-- Group list -->
	{#if loading}
		<p class="text-surface-500 text-sm">Loading…</p>
	{:else if groups.length === 0}
		<div class="card p-8 text-center text-surface-500">
			<UsersRound size={32} class="mx-auto mb-3 opacity-40" />
			<p>No player groups yet. Create one above.</p>
		</div>
	{:else}
		<div class="card overflow-hidden">
			<table class="w-full text-sm">
				<thead>
					<tr class="border-b border-surface-800 text-left text-xs font-medium uppercase tracking-wider text-surface-500">
						<th class="px-4 py-3">Name</th>
						<th class="px-4 py-3">Description</th>
						<th class="px-4 py-3 text-right">Actions</th>
					</tr>
				</thead>
				<tbody class="divide-y divide-surface-800/50">
					{#each groups as g}
						<tr class="hover:bg-surface-800/30 transition-colors">
							<td class="px-4 py-3 font-medium">
								<a href="/player-groups/{g.id}" class="text-accent hover:underline flex items-center gap-1">
									{g.name}
									<ChevronRight size={14} class="opacity-50" />
								</a>
							</td>
							<td class="px-4 py-3 text-surface-400">{g.description || '—'}</td>
							<td class="px-4 py-3 text-right">
								<button
									onclick={() => remove(g.id, g.name)}
									class="p-1 text-surface-500 hover:text-red-400 transition-colors"
									title="Delete group"
								>
									<Trash2 size={15} />
								</button>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</div>
