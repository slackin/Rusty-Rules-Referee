<script>
	import { onMount } from 'svelte';
	import { api } from '$lib/api.js';
	import { UserPlus, Trash2, Save } from 'lucide-svelte';

	let users = $state([]);
	let loading = $state(true);
	let showCreate = $state(false);
	let newUser = $state({ username: '', password: '', role: 'admin' });
	let error = $state('');
	let creating = $state(false);

	onMount(async () => {
		await loadUsers();
	});

	async function loadUsers() {
		loading = true;
		try {
			users = await api.users();
		} catch (e) {
			console.error(e);
		}
		loading = false;
	}

	async function create() {
		if (newUser.password.length < 6) { error = 'Password must be at least 6 characters'; return; }
		creating = true;
		error = '';
		try {
			await api.createUser(newUser);
			showCreate = false;
			newUser = { username: '', password: '', role: 'admin' };
			await loadUsers();
		} catch (e) {
			error = e.message;
		}
		creating = false;
	}

	async function deleteUser(id, username) {
		if (!confirm(`Delete admin user "${username}"?`)) return;
		try {
			await api.deleteUser(id);
			await loadUsers();
		} catch (e) {
			alert(e.message);
		}
	}
</script>

<div class="space-y-6 animate-fade-in">
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-2xl font-semibold">Admin Users</h1>
			<p class="mt-1 text-sm text-surface-500">Manage web dashboard access</p>
		</div>
		<button class="btn-primary btn-sm" onclick={() => showCreate = true}>
			<UserPlus class="h-3.5 w-3.5" /> Add User
		</button>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
		</div>
	{:else}
		<div class="card overflow-hidden">
			<table class="w-full text-sm">
				<thead>
					<tr class="border-b border-surface-800 text-left text-xs font-medium uppercase tracking-wider text-surface-500">
						<th class="px-5 py-3">Username</th>
						<th class="px-5 py-3">Role</th>
						<th class="px-5 py-3">Created</th>
						<th class="px-5 py-3 text-right">Actions</th>
					</tr>
				</thead>
				<tbody class="divide-y divide-surface-800/50">
					{#each users as u}
						<tr class="hover:bg-surface-800/30 transition-colors">
							<td class="px-5 py-3 font-medium text-surface-200">{u.username}</td>
							<td class="px-5 py-3"><span class="badge-blue">{u.role}</span></td>
							<td class="px-5 py-3 text-xs text-surface-500">{u.created_at ? new Date(u.created_at).toLocaleDateString() : '—'}</td>
							<td class="px-5 py-3 text-right">
								<button class="btn-ghost btn-sm text-red-400 hover:text-red-300" onclick={() => deleteUser(u.id, u.username)} title="Delete">
									<Trash2 class="h-3.5 w-3.5" />
								</button>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</div>

<!-- Create Modal -->
{#if showCreate}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onclick={() => showCreate = false}>
		<div class="card w-full max-w-md p-6 animate-slide-up" onclick={(e) => e.stopPropagation()}>
			<h3 class="text-lg font-semibold">Create Admin User</h3>
			{#if error}
				<div class="mt-3 rounded-lg bg-red-500/10 px-4 py-3 text-sm text-red-400 ring-1 ring-red-500/20">{error}</div>
			{/if}
			<div class="mt-4 space-y-3">
				<div>
					<label class="mb-1 block text-xs text-surface-500">Username</label>
					<input class="input" bind:value={newUser.username} placeholder="username" />
				</div>
				<div>
					<label class="mb-1 block text-xs text-surface-500">Password</label>
					<input class="input" type="password" bind:value={newUser.password} placeholder="min 6 characters" />
				</div>
				<div>
					<label class="mb-1 block text-xs text-surface-500">Role</label>
					<select class="input" bind:value={newUser.role}>
						<option value="admin">admin</option>
						<option value="moderator">moderator</option>
					</select>
				</div>
			</div>
			<div class="mt-4 flex justify-end gap-2">
				<button class="btn-secondary btn-sm" onclick={() => showCreate = false}>Cancel</button>
				<button class="btn-primary btn-sm" onclick={create} disabled={creating || !newUser.username || !newUser.password}>Create</button>
			</div>
		</div>
	</div>
{/if}
