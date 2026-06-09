<script>
	import { api } from '$lib/api.svelte.js';
	import { page } from '$app/stores';
	import { UsersRound, Plus, Trash2, X, ChevronRight } from 'lucide-svelte';
	import { stripColors } from '$lib/utils.js';

	let serverId = $derived(Number($page.params.id));

	let users = $state([]);
	let assignedGroups = $state([]);
	let allGroups = $state([]);
	let error = $state('');
	let loading = $state(true);
	let addingGroup = $state(false);
	let selectedGroupId = $state('');

	const LEVELS = [
		{ label: 'Guest (0)', bits: 1 },
		{ label: 'User (1)', bits: 2 },
		{ label: 'Regular (2)', bits: 4 },
		{ label: 'Moderator (20)', bits: 256 },
		{ label: 'Admin (40)', bits: 65536 },
		{ label: 'FullAdmin (60)', bits: 1073741824 },
		{ label: 'SuperAdmin (100)', bits: 9223372036854775808 },
	];

	function levelLabel(bits) {
		for (let i = LEVELS.length - 1; i >= 0; i--) {
			if (Number(bits) >= Number(LEVELS[i].bits)) return LEVELS[i].label;
		}
		return 'Guest (0)';
	}

	function badgeColor(bits) {
		const n = Number(bits);
		if (n >= 1073741824) return 'badge-red';
		if (n >= 65536) return 'badge-orange';
		if (n >= 256) return 'badge-yellow';
		if (n >= 4) return 'badge-blue';
		return 'badge-gray';
	}

	async function load() {
		loading = true;
		error = '';
		try {
			[users, assignedGroups, allGroups] = await Promise.all([
				api.serverUsers(serverId),
				api.serverPlayerGroups(serverId),
				api.playerGroups(),
			]);
		} catch (e) {
			error = e.message;
		} finally {
			loading = false;
		}
	}

	async function assignGroup() {
		if (!selectedGroupId) return;
		addingGroup = true;
		try {
			const currentIds = assignedGroups.map(g => g.id);
			const newId = Number(selectedGroupId);
			if (!currentIds.includes(newId)) {
				await api.setServerPlayerGroups(serverId, [...currentIds, newId]);
			}
			selectedGroupId = '';
			await load();
		} catch (e) {
			error = e.message;
		} finally {
			addingGroup = false;
		}
	}

	async function removeGroup(gid) {
		try {
			const newIds = assignedGroups.filter(g => g.id !== gid).map(g => g.id);
			await api.setServerPlayerGroups(serverId, newIds);
			await load();
		} catch (e) {
			error = e.message;
		}
	}

	$effect(() => { if (serverId) load(); });

	let localUsers = $derived(users.filter(u => u.source === 'Local'));
	let groupUsers = $derived(users.filter(u => u.source !== 'Local'));
	let unassignedGroups = $derived(allGroups.filter(g => !assignedGroups.some(a => a.id === g.id)));
</script>

<div class="space-y-6">
	<div class="flex items-center gap-2">
		<UsersRound size={18} class="text-accent" />
		<h2 class="text-lg font-semibold">Users & Permissions</h2>
	</div>

	{#if error}<div class="text-red-400 text-sm mb-3">{error}</div>{/if}

	<!-- Assigned Groups -->
	<div class="card p-4">
		<h3 class="text-sm font-semibold uppercase tracking-wide text-surface-500 mb-3">
			Assigned Player Groups
		</h3>
		<p class="text-xs text-surface-400 mb-3">
			This server inherits permissions from the groups below, merged with its local records.
			Group members with higher permission levels always take precedence.
		</p>

		<div class="flex flex-wrap gap-2 mb-3">
			{#each assignedGroups as g}
				<span class="inline-flex items-center gap-1 px-2 py-1 rounded bg-accent/20 text-accent text-sm font-medium">
					<a href="/player-groups/{g.id}" class="hover:underline">{g.name}</a>
					<button onclick={() => removeGroup(g.id)} class="hover:text-red-400 ml-1" title="Remove group">
						<X size={13} />
					</button>
				</span>
			{/each}
			{#if assignedGroups.length === 0}
				<span class="text-surface-500 text-sm italic">No groups assigned — using local records only.</span>
			{/if}
		</div>

		<!-- Assign group dropdown -->
		{#if unassignedGroups.length > 0}
			<div class="flex gap-2 items-center">
				<select bind:value={selectedGroupId} class="input text-sm py-1">
					<option value="">Add a player group…</option>
					{#each unassignedGroups as g}
						<option value={g.id}>{g.name}</option>
					{/each}
				</select>
				<button
					onclick={assignGroup}
					disabled={addingGroup || !selectedGroupId}
					class="btn-accent flex items-center gap-1 text-sm px-3 py-1"
				>
					<Plus size={14} /> Assign
				</button>
			</div>
		{/if}
	</div>

	{#if loading}
		<p class="text-surface-500 text-sm">Loading…</p>
	{:else}
		<!-- Effective Users table -->
		<div>
			<h3 class="text-sm font-semibold uppercase tracking-wide text-surface-500 mb-2">
				Effective Users ({users.length})
			</h3>
			<p class="text-xs text-surface-400 mb-3">
				Combined view of all group members and local records for this server.
				The "Source" column shows where the permission comes from.
			</p>

			{#if users.length === 0}
				<div class="card p-6 text-center text-surface-500 text-sm">
					<UsersRound size={24} class="mx-auto mb-2 opacity-40" />
					No users with permissions on this server yet.
				</div>
			{:else}
				<div class="card overflow-x-auto">
					<table class="w-full text-sm">
						<thead>
							<tr class="border-b border-surface-800 text-left text-xs font-medium uppercase tracking-wider text-surface-500">
								<th class="px-4 py-3">Player</th>
								<th class="px-4 py-3">Permission Level</th>
								<th class="px-4 py-3">Source</th>
							</tr>
						</thead>
						<tbody class="divide-y divide-surface-800/50">
							{#each users as u}
								<tr class="hover:bg-surface-800/20 transition-colors">
									<td class="px-4 py-2.5">
										<div class="font-medium">{u.client_name ? stripColors(u.client_name) : '(unknown)'}</div>
										<div class="font-mono text-xs text-surface-500">{u.client_guid}</div>
									</td>
									<td class="px-4 py-2.5">
										<span class="badge {badgeColor(u.group_bits)}">{levelLabel(u.group_bits)}</span>
									</td>
									<td class="px-4 py-2.5">
										{#if u.source === 'Local'}
											<span class="text-surface-400 text-xs">Local</span>
										{:else}
											<a
												href="/player-groups/{u.player_group_id}"
												class="text-accent text-xs hover:underline flex items-center gap-0.5"
											>
												{u.source}
												<ChevronRight size={12} class="opacity-60" />
											</a>
										{/if}
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		</div>

		<!-- Local overrides section -->
		{#if localUsers.length > 0}
			<div>
				<h3 class="text-sm font-semibold uppercase tracking-wide text-surface-500 mb-2">
					Local Records ({localUsers.length})
				</h3>
				<p class="text-xs text-surface-400 mb-3">
					Players with permissions set directly on this server (not from any group).
					Edit these via the Players tab → individual player page.
				</p>
				<div class="card overflow-x-auto">
					<table class="w-full text-sm">
						<thead>
							<tr class="border-b border-surface-800 text-left text-xs font-medium uppercase tracking-wider text-surface-500">
								<th class="px-4 py-3">Player</th>
								<th class="px-4 py-3">Permission Level</th>
							</tr>
						</thead>
						<tbody class="divide-y divide-surface-800/50">
							{#each localUsers as u}
								<tr class="hover:bg-surface-800/20 transition-colors">
									<td class="px-4 py-2.5">
										<div class="font-medium">{u.client_name ? stripColors(u.client_name) : '(unknown)'}</div>
										<div class="font-mono text-xs text-surface-500">{u.client_guid}</div>
									</td>
									<td class="px-4 py-2.5">
										<span class="badge {badgeColor(u.group_bits)}">{levelLabel(u.group_bits)}</span>
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</div>
		{/if}
	{/if}
</div>
