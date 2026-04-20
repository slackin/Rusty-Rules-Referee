<script>
	import { onMount } from 'svelte';
	import { checkAuth, getAuth, logout } from '$lib/auth.svelte.js';
	import { api } from '$lib/api.svelte.js';
	import { connectWs, disconnectWs } from '$lib/ws.js';
	import { initLiveStore } from '$lib/live.svelte.js';
	import {
		LayoutDashboard, Users, Shield, Gavel, Terminal, BarChart3, Settings,
		UserCog, LogOut, Menu, X, ChevronDown, KeyRound, ScrollText, MessageSquare,
		Map, BookOpen, Sliders, History
	} from 'lucide-svelte';

	let { children } = $props();
	let auth = getAuth();
	let sidebarOpen = $state(false);
	let currentPath = $state('');
	let showPasswordModal = $state(false);
	let currentPassword = $state('');
	let newPassword = $state('');
	let confirmPassword = $state('');
	let pwError = $state('');
	let pwSuccess = $state('');
	let pwLoading = $state(false);

	const nav = [
		{ href: '/', label: 'Dashboard', icon: LayoutDashboard },
		{ href: '/players', label: 'Players', icon: Users },
		{ href: '/player-history', label: 'Player History', icon: History },
		{ href: '/penalties', label: 'Penalties', icon: Gavel },
		{ href: '/chat', label: 'Chat Logs', icon: MessageSquare },
		{ href: '/stats', label: 'Statistics', icon: BarChart3 },
		{ href: '/console', label: 'Console', icon: Terminal },
		{ href: '/mapcycle', label: 'Mapcycle', icon: Map },
		{ href: '/map-config', label: 'Map Config', icon: Sliders },
		{ href: '/commands', label: 'Commands', icon: BookOpen },
		{ href: '/audit-log', label: 'Audit Log', icon: ScrollText },
		{ href: '/config', label: 'Configuration', icon: Settings },
		{ href: '/admin-users', label: 'Admin Users', icon: UserCog }
	];

	onMount(async () => {
		currentPath = window.location.pathname;
		const ok = await checkAuth();
		if (!ok) {
			window.location.href = '/login';
			return;
		}
		connectWs();
		const cleanupLive = initLiveStore();
		return () => { cleanupLive(); disconnectWs(); };
	});

	function isActive(href) {
		if (href === '/') return currentPath === '/';
		return currentPath.startsWith(href);
	}

	function openPasswordModal() {
		currentPassword = '';
		newPassword = '';
		confirmPassword = '';
		pwError = '';
		pwSuccess = '';
		showPasswordModal = true;
	}

	async function handleChangePassword() {
		pwError = '';
		pwSuccess = '';
		if (newPassword.length < 6) { pwError = 'New password must be at least 6 characters'; return; }
		if (newPassword !== confirmPassword) { pwError = 'Passwords do not match'; return; }
		pwLoading = true;
		try {
			const res = await api.changePassword(currentPassword, newPassword);
			if (res.error) { pwError = res.error; }
			else { pwSuccess = 'Password changed successfully'; setTimeout(() => { showPasswordModal = false; }, 1200); }
		} catch { pwError = 'Failed to change password'; }
		pwLoading = false;
	}
</script>

<svelte:head>
	<title>R3 Admin</title>
</svelte:head>

{#if auth.loading}
	<div class="flex min-h-screen items-center justify-center">
		<div class="h-8 w-8 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
	</div>
{:else if auth.user}
	<div class="flex min-h-screen">
		<!-- Sidebar -->
		<aside class="fixed inset-y-0 left-0 z-50 w-64 transform border-r border-surface-800 bg-surface-950 transition-transform duration-200 lg:translate-x-0 {sidebarOpen ? 'translate-x-0' : '-translate-x-full'}">
			<div class="flex h-full flex-col">
				<!-- Logo -->
				<div class="flex h-16 items-center gap-3 border-b border-surface-800 px-6">
					<div class="flex h-8 w-8 items-center justify-center rounded-lg bg-accent/10">
						<span class="text-sm font-bold text-accent">R3</span>
					</div>
					<div>
						<div class="text-sm font-semibold text-surface-100">R3 Admin</div>
						<div class="text-xs text-surface-500">Server Control</div>
					</div>
					<button class="ml-auto lg:hidden" onclick={() => sidebarOpen = false}>
						<X class="h-5 w-5 text-surface-400" />
					</button>
				</div>

				<!-- Nav -->
				<nav class="flex-1 overflow-y-auto px-3 py-4">
					<ul class="space-y-1">
						{#each nav as item}
							<li>
								<a
									href={item.href}
									class="flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors
										{isActive(item.href) ? 'bg-accent/10 text-accent font-medium' : 'text-surface-400 hover:bg-surface-800/50 hover:text-surface-200'}"
									onclick={() => { currentPath = item.href; sidebarOpen = false; }}
								>
									<item.icon class="h-4 w-4 flex-shrink-0" />
									{item.label}
								</a>
							</li>
						{/each}
					</ul>
				</nav>

				<!-- User -->
				<div class="border-t border-surface-800 p-4">
					<div class="flex items-center gap-3">
						<div class="flex h-8 w-8 items-center justify-center rounded-full bg-surface-800 text-xs font-medium text-surface-300">
							{auth.user.username?.[0]?.toUpperCase() ?? '?'}
						</div>
						<div class="flex-1 min-w-0">
							<div class="truncate text-sm font-medium text-surface-200">{auth.user.username}</div>
							<div class="text-xs text-surface-500">{auth.user.role}</div>
						</div>
						<button onclick={openPasswordModal} class="rounded-lg p-1.5 text-surface-500 hover:bg-surface-800 hover:text-surface-300" title="Change password">
							<KeyRound class="h-4 w-4" />
						</button>
						<button onclick={logout} class="rounded-lg p-1.5 text-surface-500 hover:bg-surface-800 hover:text-surface-300" title="Sign out">
							<LogOut class="h-4 w-4" />
						</button>
					</div>
				</div>
			</div>
		</aside>

		<!-- Overlay -->
		{#if sidebarOpen}
			<div class="fixed inset-0 z-40 bg-black/50 lg:hidden" onclick={() => sidebarOpen = false}></div>
		{/if}

		<!-- Main -->
		<div class="flex-1 lg:pl-64">
			<!-- Top bar -->
			<header class="sticky top-0 z-30 flex h-16 items-center gap-4 border-b border-surface-800 bg-surface-950/80 px-6 backdrop-blur-lg">
				<button class="lg:hidden" onclick={() => sidebarOpen = true}>
					<Menu class="h-5 w-5 text-surface-400" />
				</button>
				<div class="flex-1"></div>
				<div class="flex items-center gap-2">
					<div class="h-2 w-2 rounded-full bg-emerald-400 animate-pulse-soft"></div>
					<span class="text-xs text-surface-500">Connected</span>
				</div>
			</header>

			<!-- Content -->
			<main class="p-6">
				{@render children()}
			</main>
		</div>
	</div>

	<!-- Change Password Modal -->
	{#if showPasswordModal}
		<div class="fixed inset-0 z-[100] flex items-center justify-center bg-black/60 backdrop-blur-sm">
			<div class="w-full max-w-md rounded-xl border border-surface-700 bg-surface-900 p-6 shadow-2xl">
				<div class="mb-5 flex items-center gap-3">
					<div class="flex h-10 w-10 items-center justify-center rounded-lg bg-accent/10">
						<KeyRound class="h-5 w-5 text-accent" />
					</div>
					<h2 class="text-lg font-semibold text-surface-100">Change Password</h2>
				</div>

				{#if pwError}
					<div class="mb-4 rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-2 text-sm text-red-400">{pwError}</div>
				{/if}
				{#if pwSuccess}
					<div class="mb-4 rounded-lg bg-emerald-500/10 border border-emerald-500/20 px-4 py-2 text-sm text-emerald-400">{pwSuccess}</div>
				{/if}

				<div class="space-y-4">
					<div>
						<label for="currentPw" class="mb-1 block text-xs font-medium text-surface-400">Current Password</label>
						<input id="currentPw" type="password" bind:value={currentPassword} class="w-full rounded-lg border border-surface-700 bg-surface-800 px-3 py-2 text-sm text-surface-100 placeholder-surface-600 focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent" placeholder="Enter current password" />
					</div>
					<div>
						<label for="newPw" class="mb-1 block text-xs font-medium text-surface-400">New Password</label>
						<input id="newPw" type="password" bind:value={newPassword} class="w-full rounded-lg border border-surface-700 bg-surface-800 px-3 py-2 text-sm text-surface-100 placeholder-surface-600 focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent" placeholder="At least 6 characters" />
					</div>
					<div>
						<label for="confirmPw" class="mb-1 block text-xs font-medium text-surface-400">Confirm New Password</label>
						<input id="confirmPw" type="password" bind:value={confirmPassword} class="w-full rounded-lg border border-surface-700 bg-surface-800 px-3 py-2 text-sm text-surface-100 placeholder-surface-600 focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent" placeholder="Repeat new password" />
					</div>
				</div>

				<div class="mt-6 flex justify-end gap-3">
					<button onclick={() => showPasswordModal = false} class="rounded-lg border border-surface-700 px-4 py-2 text-sm text-surface-300 hover:bg-surface-800">Cancel</button>
					<button onclick={handleChangePassword} disabled={pwLoading} class="rounded-lg bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent/90 disabled:opacity-50">
						{pwLoading ? 'Saving...' : 'Change Password'}
					</button>
				</div>
			</div>
		</div>
	{/if}
{/if}
