<script>
	import { GAMETYPES, GEAR_ITEMS } from '$lib/urt-cvars.js';

	// Bindable working copy of a MapConfig / MapConfigDefault object.
	// Parent owns the state (including map_name + source), the editor
	// just mutates fields on it.
	let {
		config = $bindable(),
		disabled = false,
	} = $props();

	// ---- gear helpers ----
	// UrT semantics: `g_gear` is a BAN list — each letter in the cvar disables
	// that item. We invert in the UI so checkboxes read "allowed": checked =
	// item is permitted (NOT in g_gear), unchecked = item is banned (IN g_gear).
	// Empty g_gear still means "allow everything".
	let bannedSet = $derived(new Set((config.g_gear || '').split('')));
	function toggleGear(code) {
		const set = new Set((config.g_gear || '').split(''));
		if (set.has(code)) set.delete(code); else set.add(code);
		// Preserve a stable ordering so diffs/UI stay consistent.
		const order = GEAR_ITEMS.map((g) => g.code);
		config.g_gear = order.filter((c) => set.has(c)).join('');
	}

	// ---- supported_gametypes helpers (CSV of ids) ----
	function supportedSet() {
		if (!config.supported_gametypes) return new Set();
		return new Set(
			config.supported_gametypes
				.split(',')
				.map((s) => s.trim())
				.filter(Boolean),
		);
	}
	function toggleGametype(id) {
		const set = supportedSet();
		if (set.has(id)) set.delete(id); else set.add(id);
		config.supported_gametypes = Array.from(set).sort((a, b) => Number(a) - Number(b)).join(',');
	}
	let supportedSetReactive = $derived(supportedSet());

	// helpers to coerce null/int Option<i32>
	function intOrNull(v) {
		if (v === '' || v === null || v === undefined) return null;
		const n = Number(v);
		return Number.isFinite(n) ? n : null;
	}
</script>

<div class="space-y-6">
	<!-- Gametype section -->
	<section class="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4">
		<h3 class="mb-3 text-sm font-semibold text-zinc-200">Gametype</h3>
		<div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
			<label class="block text-xs">
				<span class="mb-1 block text-zinc-400">Default gametype</span>
				<select
					class="w-full rounded border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-sm"
					bind:value={config.default_gametype}
					{disabled}
				>
					<option value={null}>— inherit from server —</option>
					{#each GAMETYPES as gt}
						<option value={gt.value}>{gt.label}</option>
					{/each}
				</select>
			</label>
			<label class="block text-xs">
				<span class="mb-1 block text-zinc-400">Legacy "gametype" (fallback)</span>
				<select
					class="w-full rounded border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-sm"
					bind:value={config.gametype}
					{disabled}
				>
					<option value="">— unset —</option>
					{#each GAMETYPES as gt}
						<option value={gt.value}>{gt.label}</option>
					{/each}
				</select>
			</label>
		</div>
		<div class="mt-3">
			<div class="mb-1 text-xs text-zinc-400">Supported gametypes (empty = all allowed)</div>
			<div class="flex flex-wrap gap-2">
				{#each GAMETYPES as gt}
					<button
						type="button"
						class="rounded border px-2 py-1 text-xs transition {supportedSetReactive.has(gt.value)
							? 'border-blue-500 bg-blue-500/20 text-blue-300'
							: 'border-zinc-700 text-zinc-400 hover:border-zinc-600'}"
						{disabled}
						onclick={() => toggleGametype(gt.value)}
					>
						{gt.label}
					</button>
				{/each}
			</div>
		</div>
	</section>

	<!-- Limits -->
	<section class="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4">
		<h3 class="mb-3 text-sm font-semibold text-zinc-200">Limits</h3>
		<div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
			<label class="text-xs">
				<span class="mb-1 block text-zinc-400">timelimit (min)</span>
				<input type="number" min="0" class="input" {disabled}
					value={config.timelimit ?? ''}
					oninput={(e) => (config.timelimit = intOrNull(e.currentTarget.value))} />
			</label>
			<label class="text-xs">
				<span class="mb-1 block text-zinc-400">fraglimit</span>
				<input type="number" min="0" class="input" {disabled}
					value={config.fraglimit ?? ''}
					oninput={(e) => (config.fraglimit = intOrNull(e.currentTarget.value))} />
			</label>
			<label class="text-xs">
				<span class="mb-1 block text-zinc-400">capturelimit</span>
				<input type="number" min="0" class="input" {disabled}
					value={config.capturelimit ?? ''}
					oninput={(e) => (config.capturelimit = intOrNull(e.currentTarget.value))} />
			</label>
			<label class="text-xs">
				<span class="mb-1 block text-zinc-400">g_maxrounds</span>
				<input type="number" min="0" class="input" {disabled}
					value={config.g_maxrounds ?? ''}
					oninput={(e) => (config.g_maxrounds = intOrNull(e.currentTarget.value))} />
			</label>
		</div>
	</section>

	<!-- Gear -->
	<section class="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4">
		<h3 class="mb-1 text-sm font-semibold text-zinc-200">Allowed weapons & gear</h3>
		<p class="mb-3 text-xs text-zinc-500">
			Checked = item is <span class="text-emerald-400">allowed</span>.
			Unchecked items are banned and go into the <code>g_gear</code> cvar.
			All checked = allow everything (empty <code>g_gear</code>).
		</p>
		<div class="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-4">
			{#each GEAR_ITEMS as item}
				<label class="flex items-center gap-2 rounded border border-zinc-800 px-2 py-1.5 text-xs hover:border-zinc-700">
					<input type="checkbox" class="accent-blue-500"
						checked={!bannedSet.has(item.code)}
						{disabled}
						onchange={() => toggleGear(item.code)} />
					<span class="inline-block w-5 text-center font-mono text-zinc-300">{item.code}</span>
					<span class="text-zinc-400">{item.label}</span>
				</label>
			{/each}
		</div>
		<div class="mt-3 text-xs text-zinc-500">
			Banned (raw <code>g_gear</code>): <code class="text-zinc-300">{config.g_gear || '(none — all allowed)'}</code>
		</div>
	</section>

	<!-- Gameplay toggles -->
	<section class="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4">
		<h3 class="mb-3 text-sm font-semibold text-zinc-200">Gameplay toggles</h3>
		<div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
			<label class="text-xs">
				<span class="mb-1 block text-zinc-400">g_friendlyfire</span>
				<select class="input" {disabled}
					value={config.g_friendlyfire ?? ''}
					onchange={(e) => (config.g_friendlyfire = intOrNull(e.currentTarget.value))}>
					<option value="">— unset —</option>
					<option value="0">0 — Off</option>
					<option value="1">1 — On</option>
					<option value="2">2 — Mirror</option>
					<option value="3">3 — Shared</option>
				</select>
			</label>
			<label class="text-xs">
				<span class="mb-1 block text-zinc-400">g_teamdamage</span>
				<select class="input" {disabled}
					value={config.g_teamdamage ?? ''}
					onchange={(e) => (config.g_teamdamage = intOrNull(e.currentTarget.value))}>
					<option value="">— unset —</option>
					<option value="0">0 — Off</option>
					<option value="1">1 — On</option>
				</select>
			</label>
			<label class="text-xs">
				<span class="mb-1 block text-zinc-400">g_suddendeath</span>
				<select class="input" {disabled}
					value={config.g_suddendeath ?? ''}
					onchange={(e) => (config.g_suddendeath = intOrNull(e.currentTarget.value))}>
					<option value="">— unset —</option>
					<option value="0">0 — Off</option>
					<option value="1">1 — On</option>
				</select>
			</label>
			<label class="text-xs">
				<span class="mb-1 block text-zinc-400">g_followstrict</span>
				<input type="number" class="input" {disabled}
					value={config.g_followstrict ?? ''}
					oninput={(e) => (config.g_followstrict = intOrNull(e.currentTarget.value))} />
			</label>
			<label class="text-xs">
				<span class="mb-1 block text-zinc-400">g_waverespawns</span>
				<input type="number" class="input" {disabled}
					value={config.g_waverespawns ?? ''}
					oninput={(e) => (config.g_waverespawns = intOrNull(e.currentTarget.value))} />
			</label>
			<label class="text-xs">
				<span class="mb-1 block text-zinc-400">g_respawndelay</span>
				<input type="number" class="input" {disabled}
					value={config.g_respawndelay ?? ''}
					oninput={(e) => (config.g_respawndelay = intOrNull(e.currentTarget.value))} />
			</label>
		</div>
	</section>

	<!-- Match mode / bomb -->
	<section class="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4">
		<h3 class="mb-3 text-sm font-semibold text-zinc-200">Match mode &amp; Bomb</h3>
		<div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
			<label class="text-xs">
				<span class="mb-1 block text-zinc-400">g_matchmode</span>
				<select class="input" {disabled}
					value={config.g_matchmode ?? ''}
					onchange={(e) => (config.g_matchmode = intOrNull(e.currentTarget.value))}>
					<option value="">— unset —</option>
					<option value="0">0 — Off</option>
					<option value="1">1 — On</option>
				</select>
			</label>
			<label class="text-xs">
				<span class="mb-1 block text-zinc-400">g_swaproles</span>
				<select class="input" {disabled}
					value={config.g_swaproles ?? ''}
					onchange={(e) => (config.g_swaproles = intOrNull(e.currentTarget.value))}>
					<option value="">— unset —</option>
					<option value="0">0 — Off</option>
					<option value="1">1 — On</option>
				</select>
			</label>
			<label class="text-xs">
				<span class="mb-1 block text-zinc-400">g_bombdefusetime</span>
				<input type="number" min="0" class="input" {disabled}
					value={config.g_bombdefusetime ?? ''}
					oninput={(e) => (config.g_bombdefusetime = intOrNull(e.currentTarget.value))} />
			</label>
			<label class="text-xs">
				<span class="mb-1 block text-zinc-400">g_bombexplodetime</span>
				<input type="number" min="0" class="input" {disabled}
					value={config.g_bombexplodetime ?? ''}
					oninput={(e) => (config.g_bombexplodetime = intOrNull(e.currentTarget.value))} />
			</label>
			<label class="text-xs">
				<span class="mb-1 block text-zinc-400">g_gravity</span>
				<input type="number" class="input" {disabled}
					value={config.g_gravity ?? ''}
					oninput={(e) => (config.g_gravity = intOrNull(e.currentTarget.value))} />
			</label>
		</div>
	</section>

	<!-- Bots / misc -->
	<section class="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4">
		<h3 class="mb-3 text-sm font-semibold text-zinc-200">Bots &amp; misc</h3>
		<div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
			<label class="text-xs">
				<span class="mb-1 block text-zinc-400">bot (bot count; 0 disables)</span>
				<input type="number" min="0" class="input" {disabled}
					bind:value={config.bot} />
			</label>
			<label class="text-xs">
				<span class="mb-1 block text-zinc-400">skiprandom</span>
				<select class="input" {disabled} bind:value={config.skiprandom}>
					<option value={0}>0 — include in random pool</option>
					<option value={1}>1 — skip from random pool</option>
				</select>
			</label>
			<label class="col-span-full text-xs">
				<span class="mb-1 block text-zinc-400">startmessage (broadcast on map start)</span>
				<input type="text" class="input" {disabled} bind:value={config.startmessage} />
			</label>
		</div>
	</section>

	<!-- Custom RCON -->
	<section class="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4">
		<h3 class="mb-1 text-sm font-semibold text-zinc-200">Custom RCON</h3>
		<p class="mb-2 text-xs text-zinc-500">One command per line. Run after all other cvars.</p>
		<textarea
			class="input h-28 font-mono text-xs"
			{disabled}
			bind:value={config.custom_commands}
			placeholder="say Welcome!&#10;set g_warmup 10"
		></textarea>
	</section>
</div>

<style>
	:global(.input) {
		width: 100%;
		border-radius: 0.25rem;
		border: 1px solid rgb(63 63 70);
		background: rgb(9 9 11);
		padding: 0.375rem 0.5rem;
		font-size: 0.8125rem;
		color: rgb(228 228 231);
	}
	:global(.input:focus) {
		outline: none;
		border-color: rgb(59 130 246);
	}
	:global(.input:disabled) {
		opacity: 0.5;
	}
</style>
