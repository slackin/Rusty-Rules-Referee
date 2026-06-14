<script>
	import { onMount, onDestroy } from 'svelte';
	import { api } from '$lib/api.svelte.js';
	import { timeAgo } from '$lib/utils.js';
	import { Bug, Trash2, Pencil, Bot, X, RefreshCw, Play, Square, Search } from 'lucide-svelte';

	let reports = $state([]);
	let loading = $state(true);
	let statusFilter = $state('');
	let search = $state('');

	// Detail / edit drawer
	let selected = $state(null); // { report, jobs }
	let editing = $state(false);
	let editDraft = $state(null);
	let saving = $state(false);
	let drawerError = $state('');

	// Models
	let models = $state([]);
	let defaultModel = $state('');
	let chosenModel = $state('');

	// Approve / job
	let approving = $state(false);
	let activeJob = $state(null);
	let jobPoll = null;

	const STATUSES = ['new', 'triaged', 'approved', 'in_progress', 'completed', 'failed', 'rejected'];
	const SEVERITIES = ['low', 'normal', 'high', 'critical'];

	const statusBadge = {
		new: 'bg-blue-500/15 text-blue-300',
		triaged: 'bg-amber-500/15 text-amber-300',
		approved: 'bg-purple-500/15 text-purple-300',
		in_progress: 'bg-cyan-500/15 text-cyan-300',
		completed: 'bg-emerald-500/15 text-emerald-300',
		failed: 'bg-red-500/15 text-red-300',
		rejected: 'bg-surface-700 text-surface-400'
	};
	const sevBadge = {
		low: 'bg-surface-700 text-surface-300',
		normal: 'bg-blue-500/15 text-blue-300',
		high: 'bg-amber-500/15 text-amber-300',
		critical: 'bg-red-500/15 text-red-300'
	};

	onMount(async () => {
		await load();
		try {
			const m = await api.aiModels();
			models = m.models ?? [];
			defaultModel = m.default ?? '';
			chosenModel = defaultModel || models[0] || '';
		} catch { /* models optional */ }
	});

	onDestroy(() => { if (jobPoll) clearInterval(jobPoll); });

	async function load() {
		loading = true;
		try {
			reports = await api.bugReports(statusFilter);
		} catch (e) {
			console.error(e);
		}
		loading = false;
	}

	let filtered = $derived(
		search
			? reports.filter(r =>
				r.title?.toLowerCase().includes(search.toLowerCase()) ||
				r.description?.toLowerCase().includes(search.toLowerCase()))
			: reports
	);

	async function openReport(id) {
		drawerError = '';
		editing = false;
		try {
			selected = await api.bugReport(id);
			// Resume polling if the latest job is active.
			const latest = selected.jobs?.[0];
			if (latest && ['queued', 'running', 'testing', 'deploying'].includes(latest.status)) {
				startJobPoll(latest.id);
			} else {
				activeJob = latest ?? null;
			}
		} catch (e) {
			drawerError = e.message || 'Failed to load report';
		}
	}

	function closeDrawer() {
		selected = null;
		editing = false;
		activeJob = null;
		if (jobPoll) { clearInterval(jobPoll); jobPoll = null; }
	}

	function startEdit() {
		editDraft = { ...selected.report };
		editing = true;
	}

	async function saveEdit() {
		saving = true;
		drawerError = '';
		try {
			await api.updateBugReport(editDraft.id, {
				title: editDraft.title,
				description: editDraft.description,
				steps: editDraft.steps,
				severity: editDraft.severity,
				status: editDraft.status,
				admin_notes: editDraft.admin_notes
			});
			editing = false;
			await openReport(editDraft.id);
			await load();
		} catch (e) {
			drawerError = e.message || 'Save failed';
		}
		saving = false;
	}

	async function removeReport(id) {
		if (!confirm('Delete this bug report?')) return;
		try {
			await api.deleteBugReport(id);
			closeDrawer();
			await load();
		} catch (e) {
			alert(e.message);
		}
	}

	async function approve() {
		if (!confirm('Approve and launch an AI fix job? This will run the agent on the build server.')) return;
		approving = true;
		drawerError = '';
		try {
			const res = await api.approveBugReport(selected.report.id, chosenModel);
			startJobPoll(res.job_id);
			await load();
		} catch (e) {
			let msg = e.message || 'Approval failed';
			try { msg = JSON.parse(msg).error || msg; } catch {}
			drawerError = msg;
		}
		approving = false;
	}

	function startJobPoll(jobId) {
		if (jobPoll) clearInterval(jobPoll);
		const tick = async () => {
			try {
				activeJob = await api.bugJob(jobId);
				if (['success', 'failed', 'cancelled'].includes(activeJob.status)) {
					clearInterval(jobPoll); jobPoll = null;
					if (selected) await openReportSilent(selected.report.id);
					await load();
				}
			} catch { /* keep polling */ }
		};
		tick();
		jobPoll = setInterval(tick, 2000);
	}

	// Refresh report+jobs without resetting an in-progress poll.
	async function openReportSilent(id) {
		try {
			const fresh = await api.bugReport(id);
			if (selected) selected = fresh;
		} catch { /* ignore */ }
	}

	async function cancelJob() {
		if (!activeJob) return;
		try {
			await api.cancelBugJob(activeJob.id);
		} catch (e) {
			alert(e.message);
		}
	}

	let jobRunning = $derived(activeJob && ['queued', 'running', 'testing', 'deploying'].includes(activeJob.status));
</script>

<svelte:head><title>Bug Reports — R3</title></svelte:head>

<div class="space-y-6 animate-fade-in">
	<div class="flex items-center justify-between">
		<div>
			<h1 class="flex items-center gap-2 text-2xl font-semibold"><Bug class="h-6 w-6 text-accent" /> Bug Reports</h1>
			<p class="mt-1 text-sm text-surface-500">Triage public reports and launch AI fixes</p>
		</div>
		<button class="btn-ghost btn-sm" onclick={load} title="Refresh"><RefreshCw class="h-4 w-4" /></button>
	</div>

	<div class="card p-4 flex flex-wrap items-center gap-3">
		<div class="relative flex-1 min-w-48">
			<Search class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-surface-500" />
			<input type="text" bind:value={search} class="input pl-10" placeholder="Search title or description…" />
		</div>
		<select class="input w-auto" bind:value={statusFilter} onchange={load}>
			<option value="">All statuses</option>
			{#each STATUSES as s}<option value={s}>{s}</option>{/each}
		</select>
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
						<th class="px-5 py-3">#</th>
						<th class="px-5 py-3">Title</th>
						<th class="px-5 py-3">Severity</th>
						<th class="px-5 py-3">Status</th>
						<th class="px-5 py-3">Reported</th>
					</tr>
				</thead>
				<tbody class="divide-y divide-surface-800/50">
					{#each filtered as r}
						<tr class="cursor-pointer hover:bg-surface-800/30 transition-colors" onclick={() => openReport(r.id)}>
							<td class="px-5 py-3 text-surface-500">{r.id}</td>
							<td class="px-5 py-3 font-medium text-surface-200 max-w-md truncate">{r.title}</td>
							<td class="px-5 py-3"><span class="rounded px-2 py-0.5 text-xs {sevBadge[r.severity] ?? 'bg-surface-700'}">{r.severity}</span></td>
							<td class="px-5 py-3"><span class="rounded px-2 py-0.5 text-xs {statusBadge[r.status] ?? 'bg-surface-700'}">{r.status}</span></td>
							<td class="px-5 py-3 text-xs text-surface-500">{timeAgo(r.created_at)}</td>
						</tr>
					{/each}
				</tbody>
			</table>
			{#if filtered.length === 0}
				<div class="px-5 py-10 text-center text-sm text-surface-500">No bug reports</div>
			{/if}
		</div>
	{/if}
</div>

<!-- Detail drawer -->
{#if selected}
	<div class="fixed inset-0 z-50 flex justify-end bg-black/50 backdrop-blur-sm" onclick={closeDrawer}>
		<div class="h-full w-full max-w-2xl overflow-y-auto bg-surface-900 border-l border-surface-800 p-6 animate-slide-up" onclick={(e) => e.stopPropagation()}>
			<div class="mb-4 flex items-start justify-between">
				<h2 class="text-lg font-semibold text-surface-100">Bug #{selected.report.id}</h2>
				<button class="btn-ghost btn-sm" onclick={closeDrawer}><X class="h-4 w-4" /></button>
			</div>

			{#if drawerError}
				<div class="mb-4 rounded-lg bg-red-500/10 px-4 py-3 text-sm text-red-400 ring-1 ring-red-500/20">{drawerError}</div>
			{/if}

			{#if editing}
				<!-- EDIT FORM -->
				<div class="space-y-3">
					<div>
						<label class="mb-1 block text-xs text-surface-500">Title</label>
						<input class="input" bind:value={editDraft.title} maxlength="200" />
					</div>
					<div class="grid grid-cols-2 gap-3">
						<div>
							<label class="mb-1 block text-xs text-surface-500">Severity</label>
							<select class="input" bind:value={editDraft.severity}>
								{#each SEVERITIES as s}<option value={s}>{s}</option>{/each}
							</select>
						</div>
						<div>
							<label class="mb-1 block text-xs text-surface-500">Status</label>
							<select class="input" bind:value={editDraft.status}>
								{#each STATUSES as s}<option value={s}>{s}</option>{/each}
							</select>
						</div>
					</div>
					<div>
						<label class="mb-1 block text-xs text-surface-500">Description</label>
						<textarea class="input min-h-24" rows="4" bind:value={editDraft.description}></textarea>
					</div>
					<div>
						<label class="mb-1 block text-xs text-surface-500">Steps</label>
						<textarea class="input min-h-24" rows="4" bind:value={editDraft.steps}></textarea>
					</div>
					<div>
						<label class="mb-1 block text-xs text-surface-500">Admin notes</label>
						<textarea class="input min-h-16" rows="2" bind:value={editDraft.admin_notes}></textarea>
					</div>
					<div class="flex justify-end gap-2">
						<button class="btn-secondary btn-sm" onclick={() => editing = false}>Cancel</button>
						<button class="btn-primary btn-sm" onclick={saveEdit} disabled={saving}>Save</button>
					</div>
				</div>
			{:else}
				<!-- DETAIL VIEW -->
				<div class="space-y-4">
					<div class="flex items-center gap-2">
						<span class="rounded px-2 py-0.5 text-xs {sevBadge[selected.report.severity] ?? 'bg-surface-700'}">{selected.report.severity}</span>
						<span class="rounded px-2 py-0.5 text-xs {statusBadge[selected.report.status] ?? 'bg-surface-700'}">{selected.report.status}</span>
						<span class="text-xs text-surface-500">{timeAgo(selected.report.created_at)}</span>
					</div>
					<h3 class="text-base font-semibold text-surface-100">{selected.report.title}</h3>
					{#if selected.report.description}
						<div>
							<div class="mb-1 text-xs font-medium uppercase tracking-wider text-surface-500">Description</div>
							<p class="whitespace-pre-wrap text-sm text-surface-300">{selected.report.description}</p>
						</div>
					{/if}
					{#if selected.report.steps}
						<div>
							<div class="mb-1 text-xs font-medium uppercase tracking-wider text-surface-500">Steps to reproduce</div>
							<p class="whitespace-pre-wrap text-sm text-surface-300">{selected.report.steps}</p>
						</div>
					{/if}
					{#if selected.report.reporter_email}
						<div class="text-xs text-surface-500">Reporter: {selected.report.reporter_email}</div>
					{/if}
					{#if selected.report.admin_notes}
						<div>
							<div class="mb-1 text-xs font-medium uppercase tracking-wider text-surface-500">Admin notes</div>
							<p class="whitespace-pre-wrap text-sm text-surface-400">{selected.report.admin_notes}</p>
						</div>
					{/if}

					<div class="flex flex-wrap gap-2 border-t border-surface-800 pt-4">
						<button class="btn-secondary btn-sm" onclick={startEdit}><Pencil class="h-3.5 w-3.5" /> Edit</button>
						<button class="btn-ghost btn-sm text-red-400 hover:text-red-300" onclick={() => removeReport(selected.report.id)}><Trash2 class="h-3.5 w-3.5" /> Delete</button>
					</div>

					<!-- AI fix panel -->
					<div class="rounded-lg border border-surface-800 bg-surface-950/40 p-4 space-y-3">
						<div class="flex items-center gap-2 text-sm font-medium text-surface-200"><Bot class="h-4 w-4 text-accent" /> AI Fix</div>

						<div class="flex flex-wrap items-end gap-2">
							<div class="flex-1 min-w-40">
								<label class="mb-1 block text-xs text-surface-500">Model</label>
								<select class="input" bind:value={chosenModel} disabled={jobRunning}>
									{#if models.length === 0}
										<option value="">(none available)</option>
									{/if}
									{#each models as m}<option value={m}>{m}</option>{/each}
								</select>
							</div>
							{#if jobRunning}
								<button class="btn-secondary btn-sm" onclick={cancelJob}><Square class="h-3.5 w-3.5" /> Cancel</button>
							{:else}
								<button class="btn-primary btn-sm" onclick={approve} disabled={approving}><Play class="h-3.5 w-3.5" /> Approve &amp; Fix</button>
							{/if}
						</div>

						{#if activeJob}
							<div class="space-y-2">
								<div class="flex items-center gap-2 text-xs text-surface-400">
									<span class="rounded px-2 py-0.5 {statusBadge[activeJob.status] ?? 'bg-surface-700'}">{activeJob.status}</span>
									{#if activeJob.branch_name}<span class="font-mono">{activeJob.branch_name}</span>{/if}
									{#if activeJob.git_commit}<span class="font-mono text-surface-500">@ {activeJob.git_commit}</span>{/if}
								</div>
								{#if activeJob.error}
									<div class="rounded bg-red-500/10 px-3 py-2 text-xs text-red-400">{activeJob.error}</div>
								{/if}
								<pre class="max-h-80 overflow-auto rounded bg-black/40 p-3 text-xs font-mono text-surface-300 whitespace-pre-wrap">{activeJob.log || '(waiting for output…)'}</pre>
							</div>
						{/if}
					</div>
				</div>
			{/if}
		</div>
	</div>
{/if}
