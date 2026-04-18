<script>
	import { onMount } from 'svelte';
	import { checkAuth, getAuth, logout } from '$lib/auth.js';
	import { connectWs, disconnectWs } from '$lib/ws.js';
	import {
		LayoutDashboard, Users, Shield, Gavel, Terminal, BarChart3, Settings,
		UserCog, LogOut, Menu, X, ChevronDown
	} from 'lucide-svelte';

	let { children } = $props();
	let auth = getAuth();
	let sidebarOpen = $state(false);
	let currentPath = $state('');

	const nav = [
		{ href: '/', label: 'Dashboard', icon: LayoutDashboard },
		{ href: '/players', label: 'Players', icon: Users },
		{ href: '/penalties', label: 'Penalties', icon: Gavel },
		{ href: '/stats', label: 'Statistics', icon: BarChart3 },
		{ href: '/console', label: 'Console', icon: Terminal },
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
		return () => disconnectWs();
	});

	function isActive(href) {
		if (href === '/') return currentPath === '/';
		return currentPath.startsWith(href);
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
{/if}
