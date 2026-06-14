<script>
	import { api } from '$lib/api.svelte.js';
	import { page } from '$app/stores';
	import { UsersRound, Plus, X, ChevronRight, ShieldAlert, Ban, Search } from 'lucide-svelte';
	import { stripColors } from '$lib/utils.js';

	let serverId = $derived(Number($page.params.id));

	let knownUsers = $state([]);
	let assignedGroups = $state([]);
	let allGroups = $state([]);
	let error = $state('');
	let loading = $state(true);
	let addingGroup = $state(false);
	let selectedGroupId = $state('');
	let search = $state('');
	let filterFlagged = $state(false);

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
		const n = Number(bits);
		if (n <= 0) return null;
		for (let i = LEVELS.length - 1; i >= 0; i--) {
			if (n >= Number(LEVELS[i].bits)) return LEVELS[i].label;
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

	const EVASION_LABEL = {
		auth: 'same auth',
		guid: 'same GUID',
		ip: 'same IP',
		alias: 'same name',
	};

	function fmtDate(s) {
		if (!s) return '—';
		const d = new Date(s);
		if (isNaN(d)) return '—';
		return d.toLocaleString();
	}

	async function load() {
		loading = true;
		error = '';
		try {
			[knownUsers, assignedGroups, allGroups] = await Promise.all([
				api.serverKnownUsers(serverId),
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

	let unassignedGroups = $derived(allGroups.filter(g => !assignedGroups.some(a => a.id === g.id)));

	let filteredUsers = $derived.by(() => {
		const q = search.trim().toLowerCase();
		return knownUsers.filter(u => {
			if (filterFlagged && !u.banned && (!u.evasion || u.evasion.length === 0)) return false;
			if (!q) return true;
			const hay = [
				u.client_name ? stripColors(u.client_name) : '',
				u.auth ?? '',
				u.client_guid ?? '',
				u.ip ?? '',
			].join(' ').toLowerCase();
			return hay.includes(q);
		});
	});

	let flaggedCount = $derived(knownUsers.filter(u => u.banned || (u.evasion && u.evasion.length)).length);
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
			This server inherits permissions from the groups below. Members of a shared
			group appear on every server the group is assigned to.
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
		<!-- Known users -->
		<div>
			<div class="flex items-center justify-between gap-3 mb-2 flex-wrap">
				<h3 class="text-sm font-semibold uppercase tracking-wide text-surface-500">
					Known Users ({knownUsers.length})
				</h3>
				<div class="flex items-center gap-2">
					{#if flaggedCount > 0}
						<button
							class="text-xs px-2 py-1 rounded border {filterFlagged ? 'bg-red-500/20 border-red-500/40 text-red-300' : 'border-surface-700 text-surface-400 hover:text-red-300'}"
							onclick={() => filterFlagged = !filterFlagged}
							title="Show only banned / flagged accounts"
						>
							<ShieldAlert size={12} class="inline -mt-0.5" /> {flaggedCount} flagged
						</button>
					{/if}
					<div class="relative">
						<Search size={13} class="absolute left-2 top-1/2 -translate-y-1/2 text-surface-500" />
						<input
							bind:value={search}
							placeholder="Search auth, name, GUID, IP…"
							class="input text-sm py-1 pl-7 w-64"
						/>
					</div>
				</div>
			</div>
			<p class="text-xs text-surface-400 mb-3">
				Every player that has connected to this server, plus members of assigned groups.
				Identity is tracked by <span class="text-surface-300 font-medium">auth</span> first, then GUID/IP for ban-evasion detection.
			</p>

			{#if filteredUsers.length === 0}
				<div class="card p-6 text-center text-surface-500 text-sm">
					<UsersRound size={24} class="mx-auto mb-2 opacity-40" />
					{knownUsers.length === 0 ? 'No users have connected to this server yet.' : 'No users match your filter.'}
				</div>
			{:else}
				<div class="card overflow-x-auto">
					<table class="w-full text-sm">
						<thead>
							<tr class="border-b border-surface-800 text-left text-xs font-medium uppercase tracking-wider text-surface-500">
								<th class="px-4 py-3">Player</th>
								<th class="px-4 py-3">Auth</th>
								<th class="px-4 py-3">Permission</th>
								<th class="px-4 py-3">Source</th>
								<th class="px-4 py-3">Last Seen</th>
								<th class="px-4 py-3">Status</th>
							</tr>
						</thead>
						<tbody class="divide-y divide-surface-800/50">
							{#each filteredUsers as u}
								<tr class="hover:bg-surface-800/20 transition-colors {u.banned ? 'bg-red-500/5' : (u.evasion && u.evasion.length ? 'bg-amber-500/5' : '')}">
									<td class="px-4 py-2.5">
										{#if u.client_id > 0}
											<a href="/players/{u.client_id}" class="font-medium hover:text-accent hover:underline">
												{u.client_name ? stripColors(u.client_name) : '(unknown)'}
											</a>
										{:else}
											<div class="font-medium">{u.client_name ? stripColors(u.client_name) : '(unknown)'}</div>
										{/if}
										<div class="font-mono text-[11px] text-surface-500">{u.client_guid}</div>
										{#if u.ip}<div class="font-mono text-[11px] text-surface-600">{u.ip}</div>{/if}
									</td>
									<td class="px-4 py-2.5">
										{#if u.auth}
											<span class="text-[11px] uppercase font-bold px-1.5 py-0.5 rounded bg-purple-500/15 text-purple-400">{u.auth}</span>
										{:else}
											<span class="text-surface-600 text-xs italic">none</span>
										{/if}
									</td>
									<td class="px-4 py-2.5">
										{#if levelLabel(u.group_bits)}
											<span class="badge {badgeColor(u.group_bits)}">{levelLabel(u.group_bits)}</span>
										{:else}
											<span class="text-surface-600 text-xs">—</span>
										{/if}
									</td>
									<td class="px-4 py-2.5">
										{#if u.source === 'Local'}
											<span class="text-surface-400 text-xs">Local</span>
										{:else if u.source === 'Seen'}
											<span class="text-surface-600 text-xs">Connected</span>
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
									<td class="px-4 py-2.5 text-xs text-surface-400">{fmtDate(u.last_seen)}</td>
									<td class="px-4 py-2.5">
										<div class="flex flex-wrap gap-1">
											{#if u.banned}
												<span class="inline-flex items-center gap-1 text-[11px] font-bold px-1.5 py-0.5 rounded bg-red-500/20 text-red-400">
													<Ban size={11} /> BANNED
												</span>
											{/if}
											{#each (u.evasion ?? []) as ev}
												<span class="inline-flex items-center gap-1 text-[11px] font-medium px-1.5 py-0.5 rounded bg-amber-500/15 text-amber-400" title="Matches a banned account by {EVASION_LABEL[ev] ?? ev}">
													<ShieldAlert size={11} /> {EVASION_LABEL[ev] ?? ev}
												</span>
											{/each}
											{#if !u.banned && (!u.evasion || u.evasion.length === 0)}
												<span class="text-surface-600 text-xs">—</span>
											{/if}
										</div>
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		</div>
	{/if}
</div>

