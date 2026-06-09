<script>
	import { api } from '$lib/api.svelte.js';
	import { page } from '$app/stores';
	import { UsersRound, Plus, Trash2, Save, ArrowLeft, Server } from 'lucide-svelte';
	import { stripColors } from '$lib/utils.js';

	let groupId = $derived(Number($page.params.id));
	let group = $state(null);
	let members = $state([]);
	let assignedServers = $state([]);
	let allGroups = $state([]);
	let error = $state('');
	let loading = $state(true);
	let editing = $state(false);
	let editName = $state('');
	let editDesc = $state('');
	let saving = $state(false);

	// Add member form
	let addGuid = $state('');
	let addBits = $state(16); // default: Admin (bit 4 → level 40)
	let addNote = $state('');
	let addError = $state('');
	let adding = $state(false);

	// Predefined permission levels matching the game's default group table
	const LEVELS = [
		{ label: 'Guest (0)', bits: 1 },
		{ label: 'User (1)', bits: 2 },
		{ label: 'Regular (2)', bits: 4 },
		{ label: 'Moderator (20)', bits: 256 },
		{ label: 'Admin (40)', bits: 65536 },
		{ label: 'FullAdmin (60)', bits: 1073741824 },
		{ label: 'SuperAdmin (100)', bits: 0x8000000000000000 },
	];

	function levelLabel(bits) {
		for (let i = LEVELS.length - 1; i >= 0; i--) {
			if (bits >= LEVELS[i].bits) return LEVELS[i].label;
		}
		return 'Guest (0)';
	}

	async function load() {
		loading = true;
		error = '';
		try {
			const r = await api.playerGroup(groupId);
			group = r.player_group;
			members = r.members ?? [];
			assignedServers = r.assigned_servers ?? [];
			editName = group.name;
			editDesc = group.description;
		} catch (e) {
			error = e.message;
		} finally {
			loading = false;
		}
	}

	async function saveEdit() {
		saving = true;
		try {
			await api.updatePlayerGroup(groupId, { name: editName, description: editDesc });
			group = { ...group, name: editName, description: editDesc };
			editing = false;
		} catch (e) {
			error = e.message;
		} finally {
			saving = false;
		}
	}

	async function addMember() {
		addError = '';
		if (!addGuid.trim()) { addError = 'GUID is required'; return; }
		adding = true;
		try {
			await api.addPlayerGroupMember(groupId, {
				client_guid: addGuid.trim(),
				group_bits: addBits,
				note: addNote.trim(),
			});
			addGuid = '';
			addNote = '';
			await load();
		} catch (e) {
			addError = e.message;
		} finally {
			adding = false;
		}
	}

	async function removeMember(guid, name) {
		if (!confirm(`Remove ${name || guid} from this group?`)) return;
		try {
			await api.deletePlayerGroupMember(groupId, guid);
			await load();
		} catch (e) {
			error = e.message;
		}
	}

	async function updateMemberBits(guid, newBits) {
		const m = members.find(m => m.client_guid === guid);
		try {
			await api.updatePlayerGroupMember(groupId, guid, {
				group_bits: Number(newBits),
				note: m?.note ?? '',
			});
			await load();
		} catch (e) {
			error = e.message;
		}
	}

	$effect(() => { if (groupId) load(); });
</script>

<div class="max-w-4xl mx-auto px-4 py-6">
	<a href="/player-groups" class="inline-flex items-center gap-1 text-sm text-surface-400 hover:text-surface-100 mb-4">
		<ArrowLeft size={14} /> Player Groups
	</a>

	{#if loading}
		<p class="text-surface-500 text-sm">Loading…</p>
	{:else if !group}
		<p class="text-red-400">Group not found.</p>
	{:else}
		<!-- Header -->
		<div class="flex items-start justify-between mb-6">
			<div class="flex items-center gap-3">
				<UsersRound size={22} class="text-accent shrink-0" />
				{#if editing}
					<div class="flex flex-col gap-2">
						<input bind:value={editName} class="input text-xl font-bold" placeholder="Group name" />
						<input bind:value={editDesc} class="input text-sm" placeholder="Description" />
						<div class="flex gap-2 mt-1">
							<button onclick={saveEdit} disabled={saving} class="btn-accent text-sm px-3 py-1 flex items-center gap-1">
								<Save size={14} /> {saving ? 'Saving…' : 'Save'}
							</button>
							<button onclick={() => editing = false} class="btn-secondary text-sm px-3 py-1">Cancel</button>
						</div>
					</div>
				{:else}
					<div>
						<h1 class="text-2xl font-bold">{group.name}</h1>
						{#if group.description}<p class="text-surface-400 text-sm mt-0.5">{group.description}</p>{/if}
					</div>
				{/if}
			</div>
			{#if !editing}
				<button onclick={() => editing = true} class="btn-secondary text-sm px-3 py-1">Edit</button>
			{/if}
		</div>

		{#if error}<div class="text-red-400 mb-4 text-sm">{error}</div>{/if}

		<div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
			<!-- Members (left / main) -->
			<div class="lg:col-span-2 space-y-4">
				<h2 class="text-sm font-semibold uppercase tracking-wide text-surface-500">Members ({members.length})</h2>

				<!-- Add member form -->
				<div class="card p-4">
					<p class="text-xs text-surface-500 mb-3">Add a player by their game GUID. The GUID must match the client's `guid` field.</p>
					<div class="flex flex-col gap-2">
						<input bind:value={addGuid} placeholder="Player GUID (e.g. 1234abcd)" class="input font-mono text-sm" />
						<div class="flex gap-2">
							<select bind:value={addBits} class="input flex-1">
								{#each LEVELS as lv}
									<option value={lv.bits}>{lv.label}</option>
								{/each}
							</select>
							<input bind:value={addNote} placeholder="Note (optional)" class="input flex-1" />
						</div>
						{#if addError}<p class="text-red-400 text-xs">{addError}</p>{/if}
						<button onclick={addMember} disabled={adding} class="btn-accent flex items-center gap-1 self-start">
							<Plus size={15} /> {adding ? 'Adding…' : 'Add Member'}
						</button>
					</div>
				</div>

				<!-- Member table -->
				{#if members.length === 0}
					<p class="text-surface-500 text-sm text-center py-4">No members yet.</p>
				{:else}
					<div class="card overflow-hidden">
						<table class="w-full text-sm">
							<thead>
								<tr class="border-b border-surface-800 text-left text-xs font-medium uppercase tracking-wider text-surface-500">
									<th class="px-4 py-2">Player</th>
									<th class="px-4 py-2">Permission Level</th>
									<th class="px-4 py-2">Note</th>
									<th class="px-4 py-2 text-right">Remove</th>
								</tr>
							</thead>
							<tbody class="divide-y divide-surface-800/50">
								{#each members as m}
									<tr class="hover:bg-surface-800/20 transition-colors">
										<td class="px-4 py-2">
											<div class="font-medium">{m.client_name ? stripColors(m.client_name) : '(unknown)'}</div>
											<div class="font-mono text-xs text-surface-500">{m.client_guid}</div>
										</td>
										<td class="px-4 py-2">
											<select
												value={m.group_bits}
												onchange={(e) => updateMemberBits(m.client_guid, e.target.value)}
												class="input py-0.5 text-xs"
											>
												{#each LEVELS as lv}
													<option value={lv.bits}>{lv.label}</option>
												{/each}
											</select>
										</td>
										<td class="px-4 py-2 text-surface-400 text-xs">{m.note || '—'}</td>
										<td class="px-4 py-2 text-right">
											<button
												onclick={() => removeMember(m.client_guid, m.client_name)}
												class="p-1 text-surface-500 hover:text-red-400 transition-colors"
											>
												<Trash2 size={14} />
											</button>
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{/if}
			</div>

			<!-- Assigned servers (right sidebar) -->
			<div class="space-y-4">
				<h2 class="text-sm font-semibold uppercase tracking-wide text-surface-500">
					Assigned Servers ({assignedServers.length})
				</h2>
				{#if assignedServers.length === 0}
					<div class="card p-4 text-surface-500 text-sm text-center">
						<Server size={20} class="mx-auto mb-2 opacity-40" />
						No servers are using this group yet.
					</div>
				{:else}
					<div class="card divide-y divide-surface-800/50">
						{#each assignedServers as srv}
							<a href="/servers/{srv.id}/users" class="flex items-center gap-2 px-4 py-3 hover:bg-surface-800/30 transition-colors text-sm">
								<Server size={14} class="text-surface-500 shrink-0" />
								<span>{srv.name || `Server #${srv.id}`}</span>
							</a>
						{/each}
					</div>
				{/if}
			</div>
		</div>
	{/if}
</div>
