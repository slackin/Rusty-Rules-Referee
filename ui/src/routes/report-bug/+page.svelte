<script>
	import { api } from '$lib/api.svelte.js';

	let title = $state('');
	let description = $state('');
	let steps = $state('');
	let severity = $state('normal');
	let reporterEmail = $state('');
	let submitting = $state(false);
	let error = $state('');
	let submitted = $state(false);

	async function handleSubmit(e) {
		e.preventDefault();
		error = '';
		if (!title.trim()) { error = 'Please enter a title.'; return; }
		submitting = true;
		try {
			await api.submitBugReport({
				title: title.trim(),
				description: description.trim(),
				steps: steps.trim(),
				severity,
				reporter_email: reporterEmail.trim() || null
			});
			submitted = true;
		} catch (err) {
			let msg = err.message || 'Submission failed';
			try { msg = JSON.parse(msg).error || msg; } catch {}
			error = msg;
		} finally {
			submitting = false;
		}
	}

	function reportAnother() {
		title = '';
		description = '';
		steps = '';
		severity = 'normal';
		reporterEmail = '';
		submitted = false;
		error = '';
	}
</script>

<svelte:head>
	<title>Report a Bug — R3</title>
</svelte:head>

<div class="flex min-h-screen items-center justify-center px-4 py-10">
	<div class="w-full max-w-xl animate-fade-in">
		<div class="mb-8 text-center">
			<div class="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-2xl bg-accent/10 ring-1 ring-accent/20">
				<span class="text-2xl font-bold text-accent">R3</span>
			</div>
			<h1 class="text-2xl font-semibold text-surface-100">Report a Bug or Request a Feature</h1>
			<p class="mt-1 text-sm text-surface-500">Tell us what's wrong or what you'd like to see. No account needed.</p>
		</div>

		{#if submitted}
			<div class="card p-8 text-center space-y-4">
				<div class="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-green-500/10 ring-1 ring-green-500/30">
					<svg class="h-6 w-6 text-green-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
						<path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
					</svg>
				</div>
				<h2 class="text-lg font-semibold text-surface-100">Thanks for the report!</h2>
				<p class="text-sm text-surface-400">Your report has been queued for review.</p>
				<button class="btn-secondary btn-sm" onclick={reportAnother}>Report another</button>
			</div>
		{:else}
			<form onsubmit={handleSubmit} class="card p-6 space-y-4">
				{#if error}
					<div class="rounded-lg bg-red-500/10 px-4 py-3 text-sm text-red-400 ring-1 ring-red-500/20">{error}</div>
				{/if}

				<div>
					<label for="title" class="mb-1.5 block text-sm font-medium text-surface-300">Title <span class="text-red-400">*</span></label>
					<input id="title" type="text" bind:value={title} class="input" placeholder="Short summary" maxlength="200" required />
				</div>

				<div>
					<label for="severity" class="mb-1.5 block text-sm font-medium text-surface-300">Severity</label>
					<select id="severity" bind:value={severity} class="input">
						<option value="low">Low — minor / cosmetic</option>
						<option value="normal">Normal</option>
						<option value="high">High — broken feature</option>
						<option value="critical">Critical — crash / data loss</option>
					</select>
				</div>

				<div>
					<label for="description" class="mb-1.5 block text-sm font-medium text-surface-300">Description</label>
					<textarea id="description" bind:value={description} class="input min-h-24" rows="4" placeholder="What happened? What did you expect?" maxlength="8000"></textarea>
				</div>

				<div>
					<label for="steps" class="mb-1.5 block text-sm font-medium text-surface-300">Steps to reproduce</label>
					<textarea id="steps" bind:value={steps} class="input min-h-24" rows="4" placeholder="1. …&#10;2. …&#10;3. …" maxlength="8000"></textarea>
				</div>

				<div>
					<label for="email" class="mb-1.5 block text-sm font-medium text-surface-300">Your email <span class="text-surface-600">(optional)</span></label>
					<input id="email" type="email" bind:value={reporterEmail} class="input" placeholder="So we can follow up" maxlength="200" />
				</div>

				<button type="submit" class="btn-primary w-full" disabled={submitting}>
					{#if submitting}
						<span class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-white/20 border-t-white"></span>
					{/if}
					Submit report
				</button>
			</form>
		{/if}

		<p class="mt-6 text-center text-xs text-surface-600">Rusty Rules Referee</p>
	</div>
</div>
