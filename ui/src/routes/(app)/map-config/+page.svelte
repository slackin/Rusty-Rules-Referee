<script>
	import { onMount } from 'svelte';
	import { api } from '$lib/api.svelte.js';
	import { Plus, Trash2, Save, X, Pencil, Copy, ChevronDown, ChevronUp, List, Search } from 'lucide-svelte';

	let configs = $state([]);
	let availableMaps = $state([]);
	let loading = $state(true);
	let message = $state('');
	let error = $state('');
	let saving = $state(false);

	// Editor state
	let editing = $state(null); // null = list view, object = editing a config
	let isNew = $state(false);

	// Game type lookup
	const GAMETYPES = [
		{ value: '0', label: 'Free For All (FFA)' },
		{ value: '1', label: 'Last Man Standing (LMS)' },
		{ value: '3', label: 'Team Death Match (TDM)' },
		{ value: '4', label: 'Team Survivor (TS)' },
		{ value: '5', label: 'Follow The Leader (FTL)' },
		{ value: '6', label: 'Capture & Hold (CAH)' },
		{ value: '7', label: 'Capture The Flag (CTF)' },
		{ value: '8', label: 'Bomb Mode (BOMB)' },
		{ value: '9', label: 'Jump Mode' },
		{ value: '10', label: 'Freeze Tag (FT)' },
		{ value: '11', label: 'Gun Game' },
	];

	// Gear codes reference
	const GEAR_ITEMS = [
		{ code: 'G', label: 'Grenades' },
		{ code: 'A', label: 'Snipers (SR-8, PSG-1)' },
		{ code: 'a', label: 'Negev' },
		{ code: 'I', label: 'SMGs (MP5K, UMP45, MAC-11)' },
		{ code: 'W', label: 'Pistols (Desert Eagle, .50)' },
		{ code: 'N', label: 'Pistols (Beretta, Colt 1911)' },
		{ code: 'E', label: 'Automatics (G36, AK-103, LR300)' },
		{ code: 'M', label: 'Shotguns (SPAS-12, Benelli)' },
		{ code: 'K', label: 'Kevlar Vest' },
		{ code: 'L', label: 'Laser Sight' },
		{ code: 'O', label: 'Medkit' },
		{ code: 'Q', label: 'Silencer' },
		{ code: 'R', label: 'Extra Ammo' },
		{ code: 'S', label: 'Helmet' },
		{ code: 'T', label: 'NVGs (Night Vision)' },
		{ code: 'U', label: 'Tactical Goggles' },
		{ code: 'V', label: 'HE Grenade' },
		{ code: 'X', label: 'Smoke Grenade' },
		{ code: 'Z', label: 'HK69 Grenade Launcher' },
	];

	function emptyConfig() {
		return {
			map_name: '',
			gametype: '',
			capturelimit: null,
			timelimit: null,
			fraglimit: null,
			g_gear: '',
			g_gravity: null,
			g_friendlyfire: null,
			g_followstrict: null,
			g_waverespawns: null,
			g_bombdefusetime: null,
			g_bombexplodetime: null,
			g_swaproles: null,
			g_maxrounds: null,
			g_matchmode: null,
			g_respawndelay: null,
			startmessage: '',
			skiprandom: 0,
			bot: 0,
			custom_commands: ''
		};
	}

	onMount(async () => {
		try {
			const [configsData, mapsData] = await Promise.all([
				api.mapConfigs(),
				api.mapList()
			]);
			configs = configsData || [];
			const list = Array.isArray(mapsData?.maps) ? mapsData.maps : [];
			availableMaps = list.map((m) => (typeof m === 'string' ? m : m.map_name)).filter(Boolean);
		} catch (e) {
			error = 'Failed to load map configs: ' + e.message;
		}
		loading = false;
	});

	function startCreate() {
		editing = emptyConfig();
		isNew = true;
	}

	function startEdit(config) {
		editing = { ...config };
		isNew = false;
	}

	function startClone(config) {
		const clone = { ...config };
		delete clone.id;
		delete clone.created_at;
		delete clone.updated_at;
		clone.map_name = clone.map_name + '_copy';
		editing = clone;
		isNew = true;
	}

	function cancelEdit() {
		editing = null;
		isNew = false;
	}

	async function saveConfig() {
		if (!editing.map_name.trim()) {
			error = 'Map name is required';
			return;
		}
		saving = true;
		error = '';
		message = '';
		try {
			if (isNew) {
				const result = await api.createMapConfig(editing);
				editing.id = result.id;
				configs = [...configs, { ...editing, id: result.id }].sort((a, b) => a.map_name.localeCompare(b.map_name));
				message = `Config for '${editing.map_name}' created.`;
			} else {
				await api.updateMapConfig(editing.id, editing);
				configs = configs.map(c => c.id === editing.id ? { ...editing } : c);
				message = `Config for '${editing.map_name}' updated.`;
			}
			editing = null;
			isNew = false;
		} catch (e) {
			error = 'Save failed: ' + e.message;
		}
		saving = false;
	}

	async function deleteConfig(config) {
		if (!confirm(`Delete config for '${config.map_name}'?`)) return;
		try {
			await api.deleteMapConfig(config.id);
			configs = configs.filter(c => c.id !== config.id);
			message = `Config for '${config.map_name}' deleted.`;
		} catch (e) {
			error = 'Delete failed: ' + e.message;
		}
	}

	$effect(() => {
		if (message || error) {
			const timer = setTimeout(() => { message = ''; error = ''; }, 5000);
			return () => clearTimeout(timer);
		}
	});

	let mapSuggestions = $derived(
		editing && editing.map_name.length > 0
			? availableMaps.filter(m => m.toLowerCase().includes(editing.map_name.toLowerCase()) && m !== editing.map_name).slice(0, 6)
			: []
	);

	let showSuggestions = $state(false);

	function getGametypeLabel(val) {
		const gt = GAMETYPES.find(g => g.value === val);
		return gt ? gt.label : val || '—';
	}

	// Gear calculator
	let showGearCalc = $state(false);
	let gearSelection = $state({});

	function openGearCalc() {
		// Parse current g_gear (a BAN list in UrT) into selection where
		// checked = allowed (NOT in g_gear), unchecked = banned.
		const banned = editing?.g_gear || '';
		const sel = {};
		for (const item of GEAR_ITEMS) {
			sel[item.code] = !banned.includes(item.code);
		}
		gearSelection = sel;
		showGearCalc = true;
	}

	function applyGear() {
		// Persist the BAN list: letters for items NOT checked.
		editing.g_gear = GEAR_ITEMS.filter(i => !gearSelection[i.code]).map(i => i.code).join('');
		showGearCalc = false;
	}

	// Map picker
	let showMapPicker = $state(false);
	let mapFilter = $state('');
	let filteredMaps = $derived(
		availableMaps
			.filter(m => !mapFilter || m.toLowerCase().includes(mapFilter.toLowerCase()))
			.filter(m => !configs.some(c => c.map_name === m))
	);

	// Collapsible sections
	let showAdvanced = $state(false);
</script>

<svelte:head><title>Map Config | R3</title></svelte:head>

<div class="p-6 max-w-4xl mx-auto">
	{#if message}
		<div class="mb-4 p-3 bg-green-500/20 border border-green-500/40 rounded-lg text-green-300 text-sm">{message}</div>
	{/if}
	{#if error}
		<div class="mb-4 p-3 bg-red-500/20 border border-red-500/40 rounded-lg text-red-300 text-sm">{error}</div>
	{/if}

	{#if editing}
		<!-- EDITOR VIEW -->
		<div class="flex items-center justify-between mb-6">
			<h1 class="text-2xl font-bold text-white">{isNew ? 'New Map Config' : `Edit: ${editing.map_name}`}</h1>
			<div class="flex gap-2">
				<button onclick={cancelEdit} class="flex items-center gap-1.5 px-3 py-2 bg-zinc-700 text-zinc-300 rounded-lg hover:bg-zinc-600 text-sm">
					<X size={14}/> Cancel
				</button>
				<button onclick={saveConfig} disabled={saving}
					class="flex items-center gap-1.5 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-500 disabled:opacity-40 text-sm font-medium">
					<Save size={14}/> {saving ? 'Saving...' : 'Save'}
				</button>
			</div>
		</div>

		<div class="space-y-6">
			<!-- Map Name -->
			<div class="bg-zinc-800/60 border border-zinc-700/50 rounded-lg p-5">
				<h2 class="text-sm font-semibold text-zinc-300 uppercase tracking-wider mb-4">Map</h2>
				<div class="relative">
					<label class="block text-xs text-zinc-400 mb-1">Map Name</label>
					<div class="flex gap-2">
						<input type="text" bind:value={editing.map_name}
							onfocus={() => showSuggestions = true}
							onblur={() => setTimeout(() => showSuggestions = false, 200)}
							placeholder="ut4_turnpike"
							class="flex-1 px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white placeholder-zinc-500 focus:outline-none focus:border-blue-500 text-sm" />
						<button onclick={() => { mapFilter = ''; showMapPicker = true; }}
							class="flex items-center gap-1.5 px-3 py-2 bg-zinc-700 border border-zinc-600 text-zinc-300 rounded-lg hover:bg-zinc-600 hover:text-white text-sm whitespace-nowrap">
							<List size={14}/> Browse Maps
						</button>
					</div>
					{#if showSuggestions && mapSuggestions.length > 0}
						<div class="absolute top-full left-0 right-0 mt-1 bg-zinc-800 border border-zinc-700 rounded-lg shadow-xl z-10 max-h-40 overflow-y-auto">
							{#each mapSuggestions as s}
								<button onclick={() => { editing.map_name = s; showSuggestions = false; }}
									class="w-full text-left px-3 py-2 text-sm text-zinc-300 hover:bg-zinc-700 hover:text-white">{s}</button>
							{/each}
						</div>
					{/if}
				</div>
			</div>

			<!-- Game Settings -->
			<div class="bg-zinc-800/60 border border-zinc-700/50 rounded-lg p-5">
				<h2 class="text-sm font-semibold text-zinc-300 uppercase tracking-wider mb-4">Game Settings</h2>
				<div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
					<div>
						<label class="block text-xs text-zinc-400 mb-1">Game Type</label>
						<select bind:value={editing.gametype}
							class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white focus:outline-none focus:border-blue-500 text-sm">
							<option value="">— Default —</option>
							{#each GAMETYPES as gt}
								<option value={gt.value}>{gt.label}</option>
							{/each}
						</select>
					</div>
					<div>
						<label class="block text-xs text-zinc-400 mb-1">Time Limit</label>
						<input type="number" bind:value={editing.timelimit} placeholder="Default"
							class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white placeholder-zinc-500 focus:outline-none focus:border-blue-500 text-sm" />
					</div>
					<div>
						<label class="block text-xs text-zinc-400 mb-1">Capture Limit</label>
						<input type="number" bind:value={editing.capturelimit} placeholder="Default"
							class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white placeholder-zinc-500 focus:outline-none focus:border-blue-500 text-sm" />
					</div>
					<div>
						<label class="block text-xs text-zinc-400 mb-1">Frag Limit</label>
						<input type="number" bind:value={editing.fraglimit} placeholder="Default"
							class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white placeholder-zinc-500 focus:outline-none focus:border-blue-500 text-sm" />
					</div>
					<div>
						<label class="block text-xs text-zinc-400 mb-1">Friendly Fire</label>
						<select bind:value={editing.g_friendlyfire}
							class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white focus:outline-none focus:border-blue-500 text-sm">
							<option value={null}>— Default —</option>
							<option value={0}>Off</option>
							<option value={1}>On</option>
							<option value={2}>On (reflect damage)</option>
						</select>
					</div>
					<div>
						<label class="block text-xs text-zinc-400 mb-1">Gravity</label>
						<input type="number" bind:value={editing.g_gravity} placeholder="800 (default)"
							class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white placeholder-zinc-500 focus:outline-none focus:border-blue-500 text-sm" />
					</div>
				</div>
			</div>

			<!-- Weapons / Gear -->
			<div class="bg-zinc-800/60 border border-zinc-700/50 rounded-lg p-5">
				<h2 class="text-sm font-semibold text-zinc-300 uppercase tracking-wider mb-4">Weapons &amp; Gear</h2>
				<div>
					<label class="block text-xs text-zinc-400 mb-1">g_gear</label>
					<div class="flex gap-2">
						<input type="text" bind:value={editing.g_gear} placeholder="e.g. GAIKWNEMLOQURSTUVXZ"
							class="flex-1 px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white placeholder-zinc-500 focus:outline-none focus:border-blue-500 text-sm font-mono" />
						<button onclick={openGearCalc}
							class="px-3 py-2 bg-blue-600/20 border border-blue-500/40 text-blue-300 rounded-lg hover:bg-blue-600/30 text-sm whitespace-nowrap">
							Gear Calculator
						</button>
					</div>
					<p class="text-xs text-zinc-500 mt-1">Allowed gear codes. Empty = all allowed.</p>
				</div>
			</div>

			<!-- Bots & Messages -->
			<div class="bg-zinc-800/60 border border-zinc-700/50 rounded-lg p-5">
				<h2 class="text-sm font-semibold text-zinc-300 uppercase tracking-wider mb-4">Bots &amp; Messages</h2>
				<div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
					<div>
						<label class="block text-xs text-zinc-400 mb-1">Bots (bot_minplayers)</label>
						<input type="number" bind:value={editing.bot} min="0"
							class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white placeholder-zinc-500 focus:outline-none focus:border-blue-500 text-sm" />
					</div>
					<div>
						<label class="block text-xs text-zinc-400 mb-1">Skip Random (exclude from vote)</label>
						<select bind:value={editing.skiprandom}
							class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white focus:outline-none focus:border-blue-500 text-sm">
							<option value={0}>No</option>
							<option value={1}>Yes</option>
						</select>
					</div>
					<div class="sm:col-span-2">
						<label class="block text-xs text-zinc-400 mb-1">Start Message</label>
						<input type="text" bind:value={editing.startmessage} placeholder="Message shown on map load"
							class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white placeholder-zinc-500 focus:outline-none focus:border-blue-500 text-sm" />
					</div>
				</div>
			</div>

			<!-- Advanced Settings -->
			<div class="bg-zinc-800/60 border border-zinc-700/50 rounded-lg">
				<button onclick={() => showAdvanced = !showAdvanced}
					class="w-full flex items-center justify-between p-5 text-left">
					<h2 class="text-sm font-semibold text-zinc-300 uppercase tracking-wider">Advanced Settings</h2>
					{#if showAdvanced}<ChevronUp size={16} class="text-zinc-400"/>{:else}<ChevronDown size={16} class="text-zinc-400"/>{/if}
				</button>
				{#if showAdvanced}
					<div class="px-5 pb-5">
						<div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
							<div>
								<label class="block text-xs text-zinc-400 mb-1">g_followstrict</label>
								<select bind:value={editing.g_followstrict}
									class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white focus:outline-none focus:border-blue-500 text-sm">
									<option value={null}>— Default —</option>
									<option value={0}>Off</option>
									<option value={1}>On</option>
								</select>
							</div>
							<div>
								<label class="block text-xs text-zinc-400 mb-1">g_waverespawns</label>
								<select bind:value={editing.g_waverespawns}
									class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white focus:outline-none focus:border-blue-500 text-sm">
									<option value={null}>— Default —</option>
									<option value={0}>Off</option>
									<option value={1}>On</option>
								</select>
							</div>
							<div>
								<label class="block text-xs text-zinc-400 mb-1">g_swaproles</label>
								<select bind:value={editing.g_swaproles}
									class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white focus:outline-none focus:border-blue-500 text-sm">
									<option value={null}>— Default —</option>
									<option value={0}>Off</option>
									<option value={1}>On</option>
								</select>
							</div>
							<div>
								<label class="block text-xs text-zinc-400 mb-1">g_matchmode</label>
								<select bind:value={editing.g_matchmode}
									class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white focus:outline-none focus:border-blue-500 text-sm">
									<option value={null}>— Default —</option>
									<option value={0}>Off</option>
									<option value={1}>On</option>
								</select>
							</div>
							<div>
								<label class="block text-xs text-zinc-400 mb-1">g_maxrounds</label>
								<input type="number" bind:value={editing.g_maxrounds} placeholder="Default"
									class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white placeholder-zinc-500 focus:outline-none focus:border-blue-500 text-sm" />
							</div>
							<div>
								<label class="block text-xs text-zinc-400 mb-1">g_respawndelay</label>
								<input type="number" bind:value={editing.g_respawndelay} placeholder="Default"
									class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white placeholder-zinc-500 focus:outline-none focus:border-blue-500 text-sm" />
							</div>
							<div>
								<label class="block text-xs text-zinc-400 mb-1">g_bombdefusetime</label>
								<input type="number" bind:value={editing.g_bombdefusetime} placeholder="Default"
									class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white placeholder-zinc-500 focus:outline-none focus:border-blue-500 text-sm" />
							</div>
							<div>
								<label class="block text-xs text-zinc-400 mb-1">g_bombexplodetime</label>
								<input type="number" bind:value={editing.g_bombexplodetime} placeholder="Default"
									class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white placeholder-zinc-500 focus:outline-none focus:border-blue-500 text-sm" />
							</div>
						</div>
					</div>
				{/if}
			</div>

			<!-- Custom RCON Commands -->
			<div class="bg-zinc-800/60 border border-zinc-700/50 rounded-lg p-5">
				<h2 class="text-sm font-semibold text-zinc-300 uppercase tracking-wider mb-4">Custom RCON Commands</h2>
				<textarea bind:value={editing.custom_commands} rows="4"
					placeholder="One command per line, e.g.&#10;g_suddendeath 1&#10;g_warmup 15"
					class="w-full px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white placeholder-zinc-500 focus:outline-none focus:border-blue-500 text-sm font-mono"></textarea>
				<p class="text-xs text-zinc-500 mt-1">Additional RCON commands executed on map change. One per line.</p>
			</div>
		</div>

		<!-- Gear Calculator Modal -->
		{#if showGearCalc}
			<div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50" onclick={() => showGearCalc = false}>
				<div class="bg-zinc-800 border border-zinc-700 rounded-xl shadow-2xl p-6 max-w-lg w-full mx-4 max-h-[80vh] overflow-y-auto" onclick={(e) => e.stopPropagation()}>
					<h3 class="text-lg font-semibold text-white mb-4">Gear Calculator</h3>
					<p class="text-xs text-zinc-400 mb-3">Check items to <span class="text-emerald-400">allow</span> them. Unchecked items are banned — they go into the <code class="font-mono">g_gear</code> cvar. All checked = empty g_gear (everything allowed).</p>
					<div class="space-y-2">
						{#each GEAR_ITEMS as item}
							<label class="flex items-center gap-3 p-2 rounded-lg hover:bg-zinc-700/50 cursor-pointer">
								<input type="checkbox" bind:checked={gearSelection[item.code]}
									class="w-4 h-4 rounded border-zinc-600 bg-zinc-900 text-blue-600 focus:ring-blue-500" />
								<span class="font-mono text-blue-400 w-5 text-center">{item.code}</span>
								<span class="text-sm text-zinc-200">{item.label}</span>
							</label>
						{/each}
					</div>
					<div class="mt-4 p-3 bg-zinc-900 rounded-lg">
						<span class="text-xs text-zinc-400">Banned (g_gear): </span>
						<span class="font-mono text-white">{GEAR_ITEMS.filter(i => !gearSelection[i.code]).map(i => i.code).join('') || '(none — all allowed)'}</span>
					</div>
					<div class="flex gap-2 mt-4 justify-end">
						<button onclick={() => showGearCalc = false} class="px-3 py-2 bg-zinc-700 text-zinc-300 rounded-lg hover:bg-zinc-600 text-sm">Cancel</button>
						<button onclick={applyGear} class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-500 text-sm font-medium">Apply</button>
					</div>
				</div>
			</div>
		{/if}

		<!-- Map Picker Modal -->
		{#if showMapPicker}
			<div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50" onclick={() => showMapPicker = false}>
				<div class="bg-zinc-800 border border-zinc-700 rounded-xl shadow-2xl p-5 max-w-md w-full mx-4 max-h-[80vh] flex flex-col" onclick={(e) => e.stopPropagation()}>
					<h3 class="text-lg font-semibold text-white mb-3">Select Map</h3>
					<div class="relative mb-3">
						<Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500" />
						<input type="text" bind:value={mapFilter} placeholder="Filter maps..."
							class="w-full pl-9 pr-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white placeholder-zinc-500 focus:outline-none focus:border-blue-500 text-sm" />
					</div>
					<div class="flex-1 overflow-y-auto min-h-0 border border-zinc-700 rounded-lg">
						{#if filteredMaps.length === 0}
							<div class="p-4 text-center text-sm text-zinc-500">{availableMaps.length === 0 ? 'No maps found on server' : 'No maps match filter'}</div>
						{:else}
							{#each filteredMaps as m}
								<button onclick={() => { editing.map_name = m; showMapPicker = false; }}
									class="w-full text-left px-3 py-2 text-sm text-zinc-300 hover:bg-blue-600/20 hover:text-white border-b border-zinc-700/50 last:border-0 transition-colors">{m}</button>
							{/each}
						{/if}
					</div>
					<div class="mt-3 flex justify-between items-center">
						<span class="text-xs text-zinc-500">{filteredMaps.length} map{filteredMaps.length !== 1 ? 's' : ''}</span>
						<button onclick={() => showMapPicker = false} class="px-3 py-1.5 bg-zinc-700 text-zinc-300 rounded-lg hover:bg-zinc-600 text-sm">Cancel</button>
					</div>
				</div>
			</div>
		{/if}

	{:else}
		<!-- LIST VIEW -->
		<div class="flex items-center justify-between mb-6">
			<div>
				<h1 class="text-2xl font-bold text-white">Map Configuration</h1>
				<p class="text-sm text-zinc-400 mt-1">Per-map server settings applied automatically on map change.</p>
			</div>
			<button onclick={startCreate}
				class="flex items-center gap-1.5 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-500 text-sm font-medium">
				<Plus size={14}/> New Config
			</button>
		</div>

		{#if loading}
			<div class="text-zinc-400 text-center py-12">Loading map configs...</div>
		{:else if configs.length === 0}
			<div class="text-center py-16">
				<div class="text-zinc-500 text-lg mb-2">No map configurations yet</div>
				<p class="text-sm text-zinc-600 mb-6">Create per-map configs to automatically apply game type, weapons, gravity, and other settings when a specific map loads.</p>
				<button onclick={startCreate}
					class="inline-flex items-center gap-1.5 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-500 text-sm font-medium">
					<Plus size={14}/> Create First Config
				</button>
			</div>
		{:else}
			<div class="space-y-2">
				{#each configs as config}
					<div class="flex items-center gap-4 px-4 py-3 bg-zinc-800/60 border border-zinc-700/50 rounded-lg group hover:border-zinc-600 transition-colors">
						<div class="flex-1 min-w-0">
							<div class="flex items-center gap-3">
								<span class="text-white font-medium text-sm">{config.map_name}</span>
								{#if config.gametype}
									<span class="text-xs px-2 py-0.5 bg-blue-500/20 text-blue-300 rounded-full">{getGametypeLabel(config.gametype)}</span>
								{/if}
							</div>
							<div class="flex gap-3 mt-1 text-xs text-zinc-500">
								{#if config.timelimit != null}<span>Time: {config.timelimit}</span>{/if}
								{#if config.capturelimit != null}<span>Cap: {config.capturelimit}</span>{/if}
								{#if config.fraglimit != null}<span>Frags: {config.fraglimit}</span>{/if}
								{#if config.g_gravity != null}<span>Gravity: {config.g_gravity}</span>{/if}
								{#if config.g_gear}<span>Gear: <code class="font-mono">{config.g_gear}</code></span>{/if}
								{#if config.g_friendlyfire != null}<span>FF: {config.g_friendlyfire ? 'On' : 'Off'}</span>{/if}
								{#if config.bot > 0}<span>Bots: {config.bot}</span>{/if}
							</div>
						</div>
						<div class="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
							<button onclick={() => startClone(config)} title="Clone"
								class="p-2 text-zinc-400 hover:text-blue-400 rounded-lg hover:bg-zinc-700"><Copy size={14}/></button>
							<button onclick={() => startEdit(config)} title="Edit"
								class="p-2 text-zinc-400 hover:text-white rounded-lg hover:bg-zinc-700"><Pencil size={14}/></button>
							<button onclick={() => deleteConfig(config)} title="Delete"
								class="p-2 text-zinc-400 hover:text-red-400 rounded-lg hover:bg-zinc-700"><Trash2 size={14}/></button>
						</div>
					</div>
				{/each}
			</div>
			<div class="mt-4 text-xs text-zinc-500">{configs.length} map config{configs.length !== 1 ? 's' : ''}</div>
		{/if}
	{/if}
</div>
