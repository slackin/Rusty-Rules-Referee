<script>
	import { onMount } from 'svelte';
	import { api } from '$lib/api.svelte.js';
	import { Save, RotateCcw, ChevronDown, ChevronRight, Eye, EyeOff, Bot, Server, Globe, Puzzle, Info, Plus, Trash2 } from 'lucide-svelte';

	let loading = $state(true);
	let saving = $state(false);
	let message = $state('');
	let messageType = $state('');

	// Config sections
	let referee = $state({});
	let server = $state({});
	let web = $state({});
	let plugins = $state([]);

	// Original snapshots for dirty detection
	let originalJson = $state('');

	// Password visibility toggles
	let showRconPassword = $state(false);
	let showJwtSecret = $state(false);

	// Expanded plugin panels
	let expandedPlugins = $state({});

	// Plugin metadata: name -> { label, description, settings: [{ key, type, label, description, default, options? }] }
	// Field types: text, textarea, number, boolean, select, string_list, key_value, task_list
	const pluginMeta = {
		admin: {
			label: 'Admin',
			description: 'Core administration commands (kick, ban, warn, etc.)',
			settings: [
				{ key: 'warn_reason', type: 'text', label: 'Default Warn Reason', description: 'Default reason used when warning a player', default: 'Server Rule Violation' },
				{ key: 'max_warnings', type: 'number', label: 'Max Warnings', description: 'Number of warnings before automatic action', default: 3 },
				{ key: 'rules', type: 'string_list', label: 'Server Rules', description: 'Rules displayed via !rules command (one per line)', default: [] },
				{ key: 'spam_messages', type: 'key_value', label: 'Spam Messages', description: 'Quick message keywords and their text (e.g. "rules" → message)', default: {} },
				{ key: 'warn_reasons', type: 'key_value_table', label: 'Warn Reasons', description: 'Predefined warn keywords with duration (mins) and reason text', default: {} },
			]
		},
		poweradminurt: {
			label: 'Power Admin URT',
			description: 'Urban Terror specific administration features',
			settings: [
				{ key: 'team_balance_enabled', type: 'boolean', label: 'Team Balance', description: 'Automatically balance teams', default: true },
				{ key: 'team_diff', type: 'number', label: 'Max Team Difference', description: 'Maximum allowed team size difference', default: 1 },
				{ key: 'rsp_enable', type: 'boolean', label: 'Radio Spam Protection', description: 'Mute players who spam radio commands', default: false },
				{ key: 'rsp_mute_duration', type: 'number', label: 'RSP Mute Duration', description: 'Mute duration in seconds', default: 2 },
				{ key: 'rsp_max_spamins', type: 'number', label: 'RSP Spam Threshold', description: 'Spam count before muting', default: 10 },
				{ key: 'rsp_falloff_rate', type: 'number', label: 'RSP Falloff Rate', description: 'Spam counter decay rate', default: 2 },
				{ key: 'full_ident_level', type: 'number', label: 'Full Ident Level', description: 'Min admin level to see IP/GUID in !ident', default: 60 },
			]
		},
		adv: {
			label: 'Advertisements',
			description: 'Rotating server advertisement messages',
			settings: [
				{ key: 'interval_secs', type: 'number', label: 'Interval (seconds)', description: 'Seconds between advertisement rotations', default: 120 },
				{ key: 'messages', type: 'string_list', label: 'Messages', description: 'Rotating advertisement messages (URT color codes supported)', default: [] },
			]
		},
		afk: {
			label: 'AFK Detection',
			description: 'Detect and handle AFK (away from keyboard) players',
			settings: [
				{ key: 'afk_threshold_secs', type: 'number', label: 'AFK Threshold (seconds)', description: 'Seconds of inactivity before player is considered AFK', default: 300 },
				{ key: 'min_players', type: 'number', label: 'Min Players', description: 'Minimum players online before AFK kicks activate', default: 4 },
				{ key: 'check_interval_secs', type: 'number', label: 'Check Interval (seconds)', description: 'How often to check for AFK players', default: 60 },
				{ key: 'move_to_spec', type: 'boolean', label: 'Move to Spectator', description: 'Move AFK players to spectator instead of kicking', default: true },
				{ key: 'afk_message', type: 'text', label: 'AFK Message', description: 'Message shown to AFK players', default: '^7AFK: You have been inactive too long' },
			]
		},
		spawnkill: {
			label: 'Spawn Kill Protection',
			description: 'Detect and punish spawn killing',
			settings: [
				{ key: 'grace_period_secs', type: 'number', label: 'Grace Period (seconds)', description: 'Protection window after spawning', default: 3 },
				{ key: 'max_spawnkills', type: 'number', label: 'Max Spawn Kills', description: 'Spawn kills before action is taken', default: 3 },
				{ key: 'action', type: 'select', label: 'Action', description: 'Punishment for exceeding spawn kill limit', default: 'warn', options: ['warn', 'kick', 'tempban'] },
				{ key: 'tempban_duration', type: 'number', label: 'Tempban Duration (minutes)', description: 'Ban duration if action is tempban', default: 5 },
			]
		},
		spree: {
			label: 'Kill Spree',
			description: 'Announce kill spree milestones',
			settings: [
				{ key: 'min_spree', type: 'number', label: 'Min Spree Count', description: 'Minimum kills for a spree announcement', default: 5 },
				{ key: 'spree_messages', type: 'key_value', label: 'Spree Messages', description: 'Kill count → announcement message (e.g. "5" → "KILLING SPREE!")', default: {} },
			]
		},
		xlrstats: {
			label: 'XLR Stats',
			description: 'Extended live ranking and statistics system',
			settings: [
				{ key: 'kill_bonus', type: 'number', label: 'Kill Bonus', description: 'Skill calculation multiplier for kills', default: 1.2, step: 0.1 },
				{ key: 'assist_bonus', type: 'number', label: 'Assist Bonus', description: 'Point multiplier for assists', default: 0.5, step: 0.1 },
				{ key: 'min_kills', type: 'number', label: 'Min Kills', description: 'Minimum kills before stats are displayed', default: 50 },
			]
		},
		makeroom: {
			label: 'Make Room',
			description: 'Reserve slots for admins by kicking lowest-level players',
			settings: [
				{ key: 'min_admin_level', type: 'number', label: 'Min Admin Level', description: 'Minimum level that triggers room-making', default: 20 },
				{ key: 'max_players', type: 'number', label: 'Max Players', description: 'Server player capacity', default: 32 },
			]
		},
		customcommands: {
			label: 'Custom Commands',
			description: 'Define custom chat commands with text responses',
			settings: [
				{ key: 'commands', type: 'key_value', label: 'Commands', description: 'Command name → response text (e.g. "rules" → "No camping, no spawn killing")', default: {} },
			]
		},
		callvote: {
			label: 'Call Vote Control',
			description: 'Control and restrict in-game voting',
			settings: [
				{ key: 'min_level', type: 'number', label: 'Min Level to Vote', description: 'Minimum player level to call votes', default: 0 },
				{ key: 'max_votes_per_round', type: 'number', label: 'Max Votes per Round', description: 'Maximum votes a player can call per round', default: 3 },
				{ key: 'blocked_votes', type: 'string_list', label: 'Blocked Vote Types', description: 'Vote types to block (e.g. "kick", "map", "gametype")', default: [] },
			]
		},
		censor: {
			label: 'Chat Censor',
			description: 'Filter bad words from chat messages',
			settings: [
				{ key: 'warn_message', type: 'text', label: 'Warning Message', description: 'Message sent to the player when censored', default: 'Watch your language!' },
				{ key: 'max_warnings', type: 'number', label: 'Max Warnings', description: 'Warnings before kicking the player', default: 3 },
				{ key: 'bad_words', type: 'string_list', label: 'Bad Words', description: 'Regex patterns for forbidden words in chat (case-insensitive)', default: [] },
				{ key: 'bad_names', type: 'string_list', label: 'Bad Names', description: 'Regex patterns for forbidden player names (case-insensitive)', default: [] },
			]
		},
		censorurt: {
			label: 'Name Censor (URT)',
			description: 'Filter offensive player names and clan tags',
			settings: [
				{ key: 'banned_names', type: 'string_list', label: 'Banned Name Patterns', description: 'Regex patterns for banned names (case-insensitive)', default: [] },
			]
		},
		spamcontrol: {
			label: 'Spam Control',
			description: 'Prevent players from spamming chat',
			settings: [
				{ key: 'max_messages', type: 'number', label: 'Max Messages', description: 'Maximum messages in the time window', default: 5 },
				{ key: 'time_window_secs', type: 'number', label: 'Time Window (seconds)', description: 'Time window for counting messages', default: 10 },
				{ key: 'max_repeats', type: 'number', label: 'Max Repeats', description: 'Maximum consecutive repeated messages', default: 3 },
			]
		},
		tk: {
			label: 'Team Kill Tracking',
			description: 'Track and punish excessive team killing',
			settings: [
				{ key: 'max_team_kills', type: 'number', label: 'Max Team Kills', description: 'Team kills per round before action', default: 5 },
				{ key: 'max_team_damage', type: 'number', label: 'Max Team Damage', description: 'Team damage per round before action', default: 300, step: 10 },
			]
		},
		welcome: {
			label: 'Welcome Messages',
			description: 'Greet players when they join the server',
			settings: [
				{ key: 'new_player_message', type: 'textarea', label: 'New Player Message', description: 'Message for first-time players. Variables: $name', default: '^7Welcome to the server, ^2$name^7! Type ^3!help^7 for commands.' },
				{ key: 'returning_player_message', type: 'textarea', label: 'Returning Player Message', description: 'Message for returning players. Variables: $name, $last_visit', default: '^7Welcome back, ^2$name^7! You were last seen ^3$last_visit^7.' },
			]
		},
		chatlogger: {
			label: 'Chat Logger',
			description: 'Log all chat messages to files',
			settings: [
				{ key: 'log_dir', type: 'text', label: 'Log Directory', description: 'Directory for chat log files', default: 'chat_logs' },
			]
		},
		stats: {
			label: 'Basic Stats',
			description: 'Track basic in-round player statistics',
			settings: []
		},
		firstkill: {
			label: 'First Kill',
			description: 'Announce the first kill of each round',
			settings: []
		},
		flagannounce: {
			label: 'Flag Announce',
			description: 'Announce CTF flag captures, returns, and drops',
			settings: []
		},
		scheduler: {
			label: 'Scheduler',
			description: 'Run actions on game events (round start, map change, etc.)',
			settings: [
				{ key: 'tasks', type: 'task_list', label: 'Scheduled Tasks', description: 'Actions triggered by game events', default: [] },
			]
		},
		mapconfig: {
			label: 'Map Config',
			description: 'Apply per-map server configurations',
			settings: [
				{ key: 'map_configs', type: 'key_value_list', label: 'Map Configs', description: 'Map name → list of RCON commands to execute on map change', default: {} },
			]
		},
		vpncheck: {
			label: 'VPN Check',
			description: 'Detect and block VPN/proxy connections',
			settings: [
				{ key: 'kick_reason', type: 'text', label: 'Kick Reason', description: 'Message shown when kicking VPN users', default: 'VPN/Proxy connections are not allowed on this server.' },
				{ key: 'blocked_ranges', type: 'string_list', label: 'Blocked IP Ranges', description: 'IP ranges to block (format: "start.ip - end.ip")', default: [] },
			]
		},
		countryfilter: {
			label: 'Country Filter',
			description: 'Allow or block connections by country',
			settings: [
				{ key: 'mode', type: 'select', label: 'Filter Mode', description: 'Allowlist only allows listed countries; blocklist blocks them', default: 'blocklist', options: ['allowlist', 'blocklist'] },
				{ key: 'kick_message', type: 'text', label: 'Kick Message', description: 'Message shown to filtered players', default: 'Your country is not allowed on this server.' },
				{ key: 'countries', type: 'string_list', label: 'Country Codes', description: 'ISO 3166-1 alpha-2 country codes (e.g. US, DE, FR)', default: [] },
			]
		},
		pingwatch: {
			label: 'Ping Watch',
			description: 'Monitor and kick high-ping players',
			settings: [
				{ key: 'max_ping', type: 'number', label: 'Max Ping (ms)', description: 'Ping threshold for kicking', default: 250 },
				{ key: 'warn_threshold', type: 'number', label: 'Warn Threshold (ms)', description: 'Ping threshold for warnings', default: 200 },
				{ key: 'max_warnings', type: 'number', label: 'Max Warnings', description: 'Warnings before kick', default: 3 },
			]
		},
		login: {
			label: 'Login',
			description: 'Require password authentication for admin commands',
			settings: [
				{ key: 'min_level', type: 'number', label: 'Min Level', description: 'Minimum admin level requiring login', default: 20 },
			]
		},
		follow: {
			label: 'Follow',
			description: 'Follow a player and receive notifications about their activity',
			settings: []
		},
		nickreg: {
			label: 'Nick Registration',
			description: 'Protect registered nicknames from impostors',
			settings: [
				{ key: 'warn_before_kick', type: 'boolean', label: 'Warn Before Kick', description: 'Warn players before kicking for nick violation', default: true },
			]
		},
		namechecker: {
			label: 'Name Checker',
			description: 'Check for forbidden names, duplicates, and name spam',
			settings: [
				{ key: 'max_name_changes', type: 'number', label: 'Max Name Changes', description: 'Maximum name changes allowed in the time window', default: 5 },
				{ key: 'name_change_window', type: 'number', label: 'Name Change Window (seconds)', description: 'Time window for counting name changes', default: 300 },
				{ key: 'check_duplicates', type: 'boolean', label: 'Check Duplicates', description: 'Kick players with duplicate names', default: true },
				{ key: 'forbidden_patterns', type: 'string_list', label: 'Forbidden Name Patterns', description: 'Regex patterns for forbidden names (case-insensitive)', default: [] },
			]
		},
		specchecker: {
			label: 'Spectator Checker',
			description: 'Kick spectators who idle too long when the server is busy',
			settings: [
				{ key: 'max_spec_time', type: 'number', label: 'Max Spec Time (seconds)', description: 'Seconds before kicking a spectator', default: 300 },
				{ key: 'min_players', type: 'number', label: 'Min Players', description: 'Only enforce when server has this many players', default: 8 },
				{ key: 'warn_interval', type: 'number', label: 'Warn Interval (seconds)', description: 'Seconds between warnings', default: 60 },
				{ key: 'immune_level', type: 'number', label: 'Immune Level', description: 'Admin level immune to spec kicks', default: 20 },
			]
		},
		headshotcounter: {
			label: 'Headshot Counter',
			description: 'Track headshot ratios and detect possible aimbots',
			settings: [
				{ key: 'warn_ratio', type: 'number', label: 'Warn Ratio', description: 'Headshot ratio threshold for warning (0.0-1.0)', default: 0.70, step: 0.01 },
				{ key: 'ban_ratio', type: 'number', label: 'Ban Ratio', description: 'Headshot ratio threshold for auto-tempban (0.0-1.0)', default: 0.85, step: 0.01 },
				{ key: 'min_kills', type: 'number', label: 'Min Kills', description: 'Minimum kills before ratio checks activate', default: 15 },
				{ key: 'ban_duration', type: 'number', label: 'Ban Duration (minutes)', description: 'Temp-ban duration when ban ratio is exceeded', default: 60 },
				{ key: 'announce_interval', type: 'number', label: 'Announce Interval', description: 'Announce headshot streaks every N headshots', default: 10 },
			]
		},
	};

	// Event types for scheduler task_list
	const eventTypes = [
		'EVT_GAME_ROUND_START',
		'EVT_GAME_ROUND_END',
		'EVT_GAME_MAP_CHANGE',
		'EVT_GAME_WARMUP',
		'EVT_GAME_EXIT',
	];

	function currentJson() {
		return JSON.stringify({ referee, server, web, plugins });
	}

	let isDirty = $derived(currentJson() !== originalJson);

	onMount(async () => {
		try {
			const data = await api.getConfig();
			const cfg = data.config || data;
			referee = cfg.referee || {};
			server = cfg.server || {};
			web = cfg.web || {};
			plugins = (cfg.plugins || []).map(p => ({ ...p, settings: p.settings || {} }));
			originalJson = JSON.stringify({ referee, server, web, plugins });
		} catch (e) {
			message = e.message;
			messageType = 'error';
		}
		loading = false;
	});

	async function save() {
		saving = true;
		message = '';
		try {
			const payload = { referee, server, web, plugins };
			await api.updateConfig(payload);
			message = 'Configuration saved successfully. Some changes may require a restart.';
			messageType = 'success';
			originalJson = currentJson();
		} catch (e) {
			message = e.message;
			messageType = 'error';
		}
		saving = false;
	}

	function reset() {
		const orig = JSON.parse(originalJson);
		referee = orig.referee;
		server = orig.server;
		web = orig.web;
		plugins = orig.plugins;
		message = '';
	}

	function togglePlugin(idx) {
		plugins[idx].enabled = !plugins[idx].enabled;
	}

	function toggleExpand(name) {
		expandedPlugins[name] = !expandedPlugins[name];
		expandedPlugins = expandedPlugins; // trigger reactivity
	}

	function getPluginSetting(plugin, key, fallback) {
		const val = plugin.settings?.[key];
		return val !== undefined ? val : fallback;
	}

	function setPluginSetting(idx, key, value) {
		if (!plugins[idx].settings) plugins[idx].settings = {};
		plugins[idx].settings[key] = value;
		plugins = plugins; // trigger reactivity
	}

	// List helpers for string_list fields
	function addListItem(idx, key) {
		if (!plugins[idx].settings) plugins[idx].settings = {};
		if (!Array.isArray(plugins[idx].settings[key])) plugins[idx].settings[key] = [];
		plugins[idx].settings[key] = [...plugins[idx].settings[key], ''];
		plugins = plugins;
	}

	function removeListItem(idx, key, itemIdx) {
		plugins[idx].settings[key] = plugins[idx].settings[key].filter((_, i) => i !== itemIdx);
		plugins = plugins;
	}

	function updateListItem(idx, key, itemIdx, value) {
		plugins[idx].settings[key][itemIdx] = value;
		plugins = plugins;
	}

	// Key-value helpers
	function getKvEntries(plugin, key) {
		const obj = plugin.settings?.[key];
		if (!obj || typeof obj !== 'object' || Array.isArray(obj)) return [];
		return Object.entries(obj);
	}

	function addKvEntry(idx, key) {
		if (!plugins[idx].settings) plugins[idx].settings = {};
		if (!plugins[idx].settings[key] || typeof plugins[idx].settings[key] !== 'object') plugins[idx].settings[key] = {};
		plugins[idx].settings[key][''] = '';
		plugins = plugins;
	}

	function removeKvEntry(idx, settingsKey, entryKey) {
		delete plugins[idx].settings[settingsKey][entryKey];
		plugins[idx].settings[settingsKey] = { ...plugins[idx].settings[settingsKey] };
		plugins = plugins;
	}

	function renameKvEntry(idx, settingsKey, oldKey, newKey) {
		if (oldKey === newKey) return;
		const obj = plugins[idx].settings[settingsKey];
		const val = obj[oldKey];
		delete obj[oldKey];
		obj[newKey] = val;
		plugins[idx].settings[settingsKey] = { ...obj };
		plugins = plugins;
	}

	// Task list helpers for scheduler
	function getTaskList(plugin) {
		return Array.isArray(plugin.settings?.tasks) ? plugin.settings.tasks : [];
	}

	function addTask(idx) {
		if (!plugins[idx].settings) plugins[idx].settings = {};
		if (!Array.isArray(plugins[idx].settings.tasks)) plugins[idx].settings.tasks = [];
		plugins[idx].settings.tasks = [...plugins[idx].settings.tasks, { event_trigger: 'EVT_GAME_ROUND_START', action_type: 'say', action_value: '' }];
		plugins = plugins;
	}

	function removeTask(idx, taskIdx) {
		plugins[idx].settings.tasks = plugins[idx].settings.tasks.filter((_, i) => i !== taskIdx);
		plugins = plugins;
	}

	function updateTask(idx, taskIdx, field, value) {
		plugins[idx].settings.tasks[taskIdx][field] = value;
		plugins = plugins;
	}

	// Map config helpers (key_value_list: key -> array of strings)
	function getMapConfigEntries(plugin) {
		const obj = plugin.settings?.map_configs;
		if (!obj || typeof obj !== 'object' || Array.isArray(obj)) return [];
		return Object.entries(obj);
	}

	function addMapConfig(idx) {
		if (!plugins[idx].settings) plugins[idx].settings = {};
		if (!plugins[idx].settings.map_configs) plugins[idx].settings.map_configs = {};
		plugins[idx].settings.map_configs[''] = [];
		plugins = plugins;
	}

	function removeMapConfig(idx, mapName) {
		delete plugins[idx].settings.map_configs[mapName];
		plugins[idx].settings.map_configs = { ...plugins[idx].settings.map_configs };
		plugins = plugins;
	}

	function renameMapConfig(idx, oldName, newName) {
		if (oldName === newName) return;
		const obj = plugins[idx].settings.map_configs;
		const val = obj[oldName];
		delete obj[oldName];
		obj[newName] = val;
		plugins[idx].settings.map_configs = { ...obj };
		plugins = plugins;
	}

	function updateMapConfigCmds(idx, mapName, text) {
		plugins[idx].settings.map_configs[mapName] = text.split('\n').filter(l => l.trim());
		plugins = plugins;
	}
</script>

<div class="space-y-6 animate-fade-in">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-2xl font-semibold">Configuration</h1>
			<p class="mt-1 text-sm text-surface-500">Manage bot, server, web, and plugin settings</p>
		</div>
		<div class="flex gap-2">
			<button class="btn-secondary btn-sm" onclick={reset} disabled={!isDirty}>
				<RotateCcw class="h-3.5 w-3.5" /> Reset
			</button>
			<button class="btn-primary btn-sm" onclick={save} disabled={saving || !isDirty}>
				{#if saving}
					<div class="h-3.5 w-3.5 animate-spin rounded-full border-2 border-white/30 border-t-white"></div>
				{:else}
					<Save class="h-3.5 w-3.5" />
				{/if}
				Save
			</button>
		</div>
	</div>

	{#if message}
		<div class="rounded-lg px-4 py-3 text-sm {messageType === 'error' ? 'bg-red-500/10 text-red-400 ring-1 ring-red-500/20' : 'bg-emerald-500/10 text-emerald-400 ring-1 ring-emerald-500/20'}">
			{message}
		</div>
	{/if}

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
		</div>
	{:else}

		<!-- Referee Section -->
		<section class="card">
			<div class="flex items-center gap-3 border-b border-surface-800 px-6 py-4">
				<div class="flex h-9 w-9 items-center justify-center rounded-lg bg-accent/10">
					<Bot class="h-4.5 w-4.5 text-accent" />
				</div>
				<div>
					<h2 class="text-sm font-semibold text-surface-100">Bot Settings</h2>
					<p class="text-xs text-surface-500">Core bot identity and logging configuration</p>
				</div>
			</div>
			<div class="grid gap-5 p-6 sm:grid-cols-2">
				<div>
					<label for="bot_name" class="mb-1.5 block text-xs font-medium text-surface-400">Bot Name</label>
					<input id="bot_name" type="text" class="input" bind:value={referee.bot_name} placeholder="Referee" />
					<p class="mt-1 text-xs text-surface-600">Display name used in server messages</p>
				</div>
				<div>
					<label for="bot_prefix" class="mb-1.5 block text-xs font-medium text-surface-400">Bot Prefix</label>
					<input id="bot_prefix" type="text" class="input font-mono" bind:value={referee.bot_prefix} placeholder="^2RRR:^3" />
					<p class="mt-1 text-xs text-surface-600">Color-coded prefix for bot messages (URT color codes)</p>
				</div>
				<div>
					<label for="database" class="mb-1.5 block text-xs font-medium text-surface-400">Database</label>
					<input id="database" type="text" class="input" bind:value={referee.database} placeholder="sqlite://referee.db" />
					<p class="mt-1 text-xs text-amber-500/80"><Info class="inline h-3 w-3 -mt-0.5" /> Requires restart to take effect</p>
				</div>
				<div>
					<label for="logfile" class="mb-1.5 block text-xs font-medium text-surface-400">Log File</label>
					<input id="logfile" type="text" class="input" bind:value={referee.logfile} placeholder="referee.log" />
					<p class="mt-1 text-xs text-surface-600">Path to the bot's log file</p>
				</div>
				<div>
					<label for="log_level" class="mb-1.5 block text-xs font-medium text-surface-400">Log Level</label>
					<select id="log_level" class="input" bind:value={referee.log_level}>
						<option value="error">Error</option>
						<option value="warn">Warn</option>
						<option value="info">Info</option>
						<option value="debug">Debug</option>
						<option value="trace">Trace</option>
					</select>
					<p class="mt-1 text-xs text-surface-600">Logging verbosity level</p>
				</div>
			</div>
		</section>

		<!-- Server Section -->
		<section class="card">
			<div class="flex items-center gap-3 border-b border-surface-800 px-6 py-4">
				<div class="flex h-9 w-9 items-center justify-center rounded-lg bg-blue-500/10">
					<Server class="h-4.5 w-4.5 text-blue-400" />
				</div>
				<div>
					<h2 class="text-sm font-semibold text-surface-100">Game Server</h2>
					<p class="text-xs text-surface-500">Connection settings for the Urban Terror game server</p>
				</div>
			</div>
			<div class="grid gap-5 p-6 sm:grid-cols-2">
				<div>
					<label for="public_ip" class="mb-1.5 block text-xs font-medium text-surface-400">Public IP</label>
					<input id="public_ip" type="text" class="input font-mono" bind:value={server.public_ip} placeholder="192.168.1.100" />
					<p class="mt-1 text-xs text-amber-500/80"><Info class="inline h-3 w-3 -mt-0.5" /> Requires restart to take effect</p>
				</div>
				<div>
					<label for="port" class="mb-1.5 block text-xs font-medium text-surface-400">Port</label>
					<input id="port" type="number" class="input font-mono" bind:value={server.port} placeholder="27960" />
					<p class="mt-1 text-xs text-amber-500/80"><Info class="inline h-3 w-3 -mt-0.5" /> Requires restart to take effect</p>
				</div>
				<div>
					<label for="rcon_ip" class="mb-1.5 block text-xs font-medium text-surface-400">RCON IP <span class="text-surface-600 font-normal">(optional)</span></label>
					<input id="rcon_ip" type="text" class="input font-mono" bind:value={server.rcon_ip} placeholder="Defaults to public IP" />
					<p class="mt-1 text-xs text-surface-600">Override IP for RCON connections</p>
				</div>
				<div>
					<label for="rcon_port" class="mb-1.5 block text-xs font-medium text-surface-400">RCON Port <span class="text-surface-600 font-normal">(optional)</span></label>
					<input id="rcon_port" type="number" class="input font-mono" bind:value={server.rcon_port} placeholder="Defaults to game port" />
					<p class="mt-1 text-xs text-surface-600">Override port for RCON connections</p>
				</div>
				<div>
					<label for="rcon_password" class="mb-1.5 block text-xs font-medium text-surface-400">RCON Password</label>
					<div class="relative">
						<input id="rcon_password" type={showRconPassword ? 'text' : 'password'} class="input font-mono pr-10" bind:value={server.rcon_password} placeholder="••••••••" />
						<button type="button" class="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-surface-500 hover:text-surface-300" onclick={() => showRconPassword = !showRconPassword}>
							{#if showRconPassword}<EyeOff class="h-4 w-4" />{:else}<Eye class="h-4 w-4" />{/if}
						</button>
					</div>
					<p class="mt-1 text-xs text-surface-600">Leave as ******** to keep current password</p>
				</div>
				<div>
					<label for="game_log" class="mb-1.5 block text-xs font-medium text-surface-400">Game Log Path <span class="text-surface-600 font-normal">(optional)</span></label>
					<input id="game_log" type="text" class="input font-mono" bind:value={server.game_log} placeholder="/path/to/games.log" />
					<p class="mt-1 text-xs text-surface-600">Path to the server's game log file</p>
				</div>
				<div>
					<label for="delay" class="mb-1.5 block text-xs font-medium text-surface-400">Log Read Delay</label>
					<input id="delay" type="number" step="0.01" min="0.1" class="input font-mono" bind:value={server.delay} placeholder="0.33" />
					<p class="mt-1 text-xs text-surface-600">Seconds between game log reads</p>
				</div>
			</div>
		</section>

		<!-- Web Section -->
		<section class="card">
			<div class="flex items-center gap-3 border-b border-surface-800 px-6 py-4">
				<div class="flex h-9 w-9 items-center justify-center rounded-lg bg-purple-500/10">
					<Globe class="h-4.5 w-4.5 text-purple-400" />
				</div>
				<div>
					<h2 class="text-sm font-semibold text-surface-100">Web Admin UI</h2>
					<p class="text-xs text-surface-500">Settings for the web administration dashboard</p>
				</div>
			</div>
			<div class="grid gap-5 p-6 sm:grid-cols-2">
				<div class="sm:col-span-2">
					<label class="flex items-center gap-3 cursor-pointer">
						<button
							type="button"
							aria-label="Toggle web UI"
							class="relative h-5 w-9 rounded-full transition-colors {web.enabled ? 'bg-accent' : 'bg-surface-700'}"
							onclick={() => web.enabled = !web.enabled}
						>
							<span class="absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-white shadow transition-transform {web.enabled ? 'translate-x-4' : ''}"></span>
						</button>
						<span class="text-sm text-surface-200">Enable Web Admin UI</span>
					</label>
				</div>
				<div>
					<label for="bind_address" class="mb-1.5 block text-xs font-medium text-surface-400">Bind Address</label>
					<input id="bind_address" type="text" class="input font-mono" bind:value={web.bind_address} placeholder="0.0.0.0" />
					<p class="mt-1 text-xs text-amber-500/80"><Info class="inline h-3 w-3 -mt-0.5" /> Requires restart to take effect</p>
				</div>
				<div>
					<label for="web_port" class="mb-1.5 block text-xs font-medium text-surface-400">Port</label>
					<input id="web_port" type="number" class="input font-mono" bind:value={web.port} placeholder="8080" />
					<p class="mt-1 text-xs text-amber-500/80"><Info class="inline h-3 w-3 -mt-0.5" /> Requires restart to take effect</p>
				</div>
				<div>
					<label for="jwt_secret" class="mb-1.5 block text-xs font-medium text-surface-400">JWT Secret <span class="text-surface-600 font-normal">(optional)</span></label>
					<div class="relative">
						<input id="jwt_secret" type={showJwtSecret ? 'text' : 'password'} class="input font-mono pr-10" bind:value={web.jwt_secret} placeholder="Auto-generated if not set" />
						<button type="button" class="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-surface-500 hover:text-surface-300" onclick={() => showJwtSecret = !showJwtSecret}>
							{#if showJwtSecret}<EyeOff class="h-4 w-4" />{:else}<Eye class="h-4 w-4" />{/if}
						</button>
					</div>
					<p class="mt-1 text-xs text-surface-600">Leave as ******** to keep current secret</p>
				</div>
			</div>
		</section>

		<!-- Plugins Section -->
		<section>
			<div class="mb-4 flex items-center gap-3">
				<div class="flex h-9 w-9 items-center justify-center rounded-lg bg-emerald-500/10">
					<Puzzle class="h-4.5 w-4.5 text-emerald-400" />
				</div>
				<div>
					<h2 class="text-sm font-semibold text-surface-100">Plugins</h2>
					<p class="text-xs text-surface-500">Enable, disable, and configure loaded plugins</p>
				</div>
			</div>

			<div class="space-y-2">
				{#each plugins as plugin, idx}
					{@const meta = pluginMeta[plugin.name] || { label: plugin.name, description: '', settings: [] }}
					{@const hasSettings = meta.settings.length > 0}
					{@const isExpanded = expandedPlugins[plugin.name]}
					<div class="card overflow-hidden">
						<!-- Plugin header -->
						<div class="flex items-center gap-3 px-5 py-3.5">
							<!-- Expand/collapse button -->
							{#if hasSettings}
								<button type="button" class="p-0.5 text-surface-500 hover:text-surface-300" onclick={() => toggleExpand(plugin.name)}>
									{#if isExpanded}<ChevronDown class="h-4 w-4" />{:else}<ChevronRight class="h-4 w-4" />{/if}
								</button>
							{:else}
								<span class="w-5"></span>
							{/if}

							<!-- Plugin info -->
							<!-- svelte-ignore a11y_no_static_element_interactions -->
							<!-- svelte-ignore a11y_click_events_have_key_events -->
							<div class="flex-1 min-w-0" class:cursor-pointer={hasSettings} onclick={() => hasSettings && toggleExpand(plugin.name)}>
								<div class="flex items-center gap-2">
									<span class="text-sm font-medium text-surface-200">{meta.label}</span>
									<span class="font-mono text-xs text-surface-600">{plugin.name}</span>
								</div>
								{#if meta.description}
									<p class="text-xs text-surface-500 truncate">{meta.description}</p>
								{/if}
							</div>

							<!-- Enabled toggle -->
							<button
								type="button"
								aria-label="Toggle {meta.label} plugin"
								class="relative h-5 w-9 flex-shrink-0 rounded-full transition-colors {plugin.enabled ? 'bg-accent' : 'bg-surface-700'}"
								onclick={() => togglePlugin(idx)}
							>
								<span class="absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-white shadow transition-transform {plugin.enabled ? 'translate-x-4' : ''}"></span>
							</button>
						</div>

						<!-- Plugin settings (collapsible) -->
						{#if hasSettings && isExpanded}
							<div class="border-t border-surface-800 bg-surface-950/30 px-5 py-4">
								<div class="grid gap-4 sm:grid-cols-2">
									{#each meta.settings as field}
										{#if field.type === 'string_list'}
											{@const items = Array.isArray(getPluginSetting(plugin, field.key, field.default)) ? getPluginSetting(plugin, field.key, field.default) : []}
											<div class="sm:col-span-2">
												<label class="mb-1.5 block text-xs font-medium text-surface-400">{field.label}</label>
												<div class="space-y-1.5">
													{#each items as item, itemIdx}
														<div class="flex gap-2">
															<input
																type="text"
																class="input flex-1 font-mono text-sm"
																value={item}
																oninput={(e) => updateListItem(idx, field.key, itemIdx, e.target.value)}
																placeholder="Enter value..."
															/>
															<button type="button" class="p-1.5 text-surface-500 hover:text-red-400" onclick={() => removeListItem(idx, field.key, itemIdx)}>
																<Trash2 class="h-3.5 w-3.5" />
															</button>
														</div>
													{/each}
												</div>
												<button type="button" class="mt-2 flex items-center gap-1 text-xs text-accent hover:text-accent/80" onclick={() => addListItem(idx, field.key)}>
													<Plus class="h-3 w-3" /> Add item
												</button>
												{#if field.description}
													<p class="mt-1 text-xs text-surface-600">{field.description}</p>
												{/if}
											</div>
										{:else if field.type === 'key_value'}
											{@const entries = getKvEntries(plugin, field.key)}
											<div class="sm:col-span-2">
												<label class="mb-1.5 block text-xs font-medium text-surface-400">{field.label}</label>
												<div class="space-y-1.5">
													{#each entries as [k, v], entryIdx}
														<div class="flex gap-2">
															<input
																type="text"
																class="input w-1/3 font-mono text-sm"
																value={k}
																placeholder="Key"
																onblur={(e) => renameKvEntry(idx, field.key, k, e.target.value)}
															/>
															<input
																type="text"
																class="input flex-1 text-sm"
																value={v}
																placeholder="Value"
																oninput={(e) => { plugins[idx].settings[field.key][k] = e.target.value; plugins = plugins; }}
															/>
															<button type="button" class="p-1.5 text-surface-500 hover:text-red-400" onclick={() => removeKvEntry(idx, field.key, k)}>
																<Trash2 class="h-3.5 w-3.5" />
															</button>
														</div>
													{/each}
												</div>
												<button type="button" class="mt-2 flex items-center gap-1 text-xs text-accent hover:text-accent/80" onclick={() => addKvEntry(idx, field.key)}>
													<Plus class="h-3 w-3" /> Add entry
												</button>
												{#if field.description}
													<p class="mt-1 text-xs text-surface-600">{field.description}</p>
												{/if}
											</div>
										{:else if field.type === 'key_value_table'}
											{@const entries = getKvEntries(plugin, field.key)}
											<div class="sm:col-span-2">
												<label class="mb-1.5 block text-xs font-medium text-surface-400">{field.label}</label>
												<div class="space-y-1.5">
													{#each entries as [k, v], entryIdx}
														{@const dur = typeof v === 'object' ? (v.duration ?? '') : ''}
														{@const reason = typeof v === 'object' ? (v.reason ?? '') : (typeof v === 'string' ? v : '')}
														<div class="flex gap-2">
															<input
																type="text"
																class="input w-1/4 font-mono text-sm"
																value={k}
																placeholder="Keyword"
																onblur={(e) => renameKvEntry(idx, field.key, k, e.target.value)}
															/>
															<input
																type="number"
																class="input w-20 font-mono text-sm"
																value={dur}
																placeholder="Mins"
																oninput={(e) => {
																	if (!plugins[idx].settings[field.key][k] || typeof plugins[idx].settings[field.key][k] !== 'object') {
																		plugins[idx].settings[field.key][k] = { duration: 0, reason: '' };
																	}
																	plugins[idx].settings[field.key][k].duration = Number(e.target.value);
																	plugins = plugins;
																}}
															/>
															<input
																type="text"
																class="input flex-1 text-sm"
																value={reason}
																placeholder="Reason text"
																oninput={(e) => {
																	if (!plugins[idx].settings[field.key][k] || typeof plugins[idx].settings[field.key][k] !== 'object') {
																		plugins[idx].settings[field.key][k] = { duration: 0, reason: '' };
																	}
																	plugins[idx].settings[field.key][k].reason = e.target.value;
																	plugins = plugins;
																}}
															/>
															<button type="button" class="p-1.5 text-surface-500 hover:text-red-400" onclick={() => removeKvEntry(idx, field.key, k)}>
																<Trash2 class="h-3.5 w-3.5" />
															</button>
														</div>
													{/each}
												</div>
												<button type="button" class="mt-2 flex items-center gap-1 text-xs text-accent hover:text-accent/80" onclick={() => { addKvEntry(idx, field.key); plugins[idx].settings[field.key][''] = { duration: 5, reason: '' }; plugins = plugins; }}>
													<Plus class="h-3 w-3" /> Add entry
												</button>
												{#if field.description}
													<p class="mt-1 text-xs text-surface-600">{field.description}</p>
												{/if}
											</div>
										{:else if field.type === 'task_list'}
											{@const tasks = getTaskList(plugin)}
											<div class="sm:col-span-2">
												<label class="mb-1.5 block text-xs font-medium text-surface-400">{field.label}</label>
												<div class="space-y-1.5">
													{#each tasks as task, taskIdx}
														<div class="flex gap-2">
															<select
																class="input w-1/3 text-sm"
																value={task.event_trigger}
																onchange={(e) => updateTask(idx, taskIdx, 'event_trigger', e.target.value)}
															>
																{#each eventTypes as evt}
																	<option value={evt}>{evt.replace('EVT_', '').replace(/_/g, ' ')}</option>
																{/each}
															</select>
															<select
																class="input w-20 text-sm"
																value={task.action_type}
																onchange={(e) => updateTask(idx, taskIdx, 'action_type', e.target.value)}
															>
																<option value="say">Say</option>
																<option value="rcon">RCON</option>
															</select>
															<input
																type="text"
																class="input flex-1 font-mono text-sm"
																value={task.action_value}
																placeholder={task.action_type === 'rcon' ? 'RCON command' : 'Message text'}
																oninput={(e) => updateTask(idx, taskIdx, 'action_value', e.target.value)}
															/>
															<button type="button" class="p-1.5 text-surface-500 hover:text-red-400" onclick={() => removeTask(idx, taskIdx)}>
																<Trash2 class="h-3.5 w-3.5" />
															</button>
														</div>
													{/each}
												</div>
												<button type="button" class="mt-2 flex items-center gap-1 text-xs text-accent hover:text-accent/80" onclick={() => addTask(idx)}>
													<Plus class="h-3 w-3" /> Add task
												</button>
												{#if field.description}
													<p class="mt-1 text-xs text-surface-600">{field.description}</p>
												{/if}
											</div>
										{:else if field.type === 'key_value_list'}
											{@const mapEntries = getMapConfigEntries(plugin)}
											<div class="sm:col-span-2">
												<label class="mb-1.5 block text-xs font-medium text-surface-400">{field.label}</label>
												<div class="space-y-3">
													{#each mapEntries as [mapName, cmds]}
														<div class="rounded-lg border border-surface-800 p-3">
															<div class="flex items-center gap-2 mb-2">
																<input
																	type="text"
																	class="input font-mono text-sm w-48"
																	value={mapName}
																	placeholder="Map name"
																	onblur={(e) => renameMapConfig(idx, mapName, e.target.value)}
																/>
																<button type="button" class="p-1.5 text-surface-500 hover:text-red-400 ml-auto" onclick={() => removeMapConfig(idx, mapName)}>
																	<Trash2 class="h-3.5 w-3.5" />
																</button>
															</div>
															<textarea
																class="input w-full font-mono text-sm"
																rows="3"
																value={Array.isArray(cmds) ? cmds.join('\n') : ''}
																placeholder="One RCON command per line"
																oninput={(e) => updateMapConfigCmds(idx, mapName, e.target.value)}
															></textarea>
														</div>
													{/each}
												</div>
												<button type="button" class="mt-2 flex items-center gap-1 text-xs text-accent hover:text-accent/80" onclick={() => addMapConfig(idx)}>
													<Plus class="h-3 w-3" /> Add map
												</button>
												{#if field.description}
													<p class="mt-1 text-xs text-surface-600">{field.description}</p>
												{/if}
											</div>
										{:else}
										<div>
											<label for="plugin_{plugin.name}_{field.key}" class="mb-1.5 block text-xs font-medium text-surface-400">{field.label}</label>
											{#if field.type === 'boolean'}
												<label class="flex items-center gap-3 cursor-pointer">
													<button
														type="button"
														id="plugin_{plugin.name}_{field.key}"
														aria-label="Toggle {field.label}"
														class="relative h-5 w-9 rounded-full transition-colors {getPluginSetting(plugin, field.key, field.default) ? 'bg-accent' : 'bg-surface-700'}"
														onclick={() => setPluginSetting(idx, field.key, !getPluginSetting(plugin, field.key, field.default))}
													>
														<span class="absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-white shadow transition-transform {getPluginSetting(plugin, field.key, field.default) ? 'translate-x-4' : ''}"></span>
													</button>
													<span class="text-xs text-surface-400">{getPluginSetting(plugin, field.key, field.default) ? 'Enabled' : 'Disabled'}</span>
												</label>
											{:else if field.type === 'select'}
												<select
													id="plugin_{plugin.name}_{field.key}"
													class="input"
													value={getPluginSetting(plugin, field.key, field.default)}
													onchange={(e) => setPluginSetting(idx, field.key, e.target.value)}
												>
													{#each field.options as opt}
														<option value={opt}>{opt}</option>
													{/each}
												</select>
											{:else if field.type === 'number'}
												<input
													id="plugin_{plugin.name}_{field.key}"
													type="number"
													step={field.step || 1}
													class="input font-mono"
													value={getPluginSetting(plugin, field.key, field.default)}
													oninput={(e) => setPluginSetting(idx, field.key, Number(e.target.value))}
												/>
											{:else if field.type === 'textarea'}
												<textarea
													id="plugin_{plugin.name}_{field.key}"
													class="input font-mono text-sm"
													rows="2"
													value={getPluginSetting(plugin, field.key, field.default)}
													oninput={(e) => setPluginSetting(idx, field.key, e.target.value)}
												></textarea>
											{:else}
												<input
													id="plugin_{plugin.name}_{field.key}"
													type="text"
													class="input"
													value={getPluginSetting(plugin, field.key, field.default)}
													oninput={(e) => setPluginSetting(idx, field.key, e.target.value)}
												/>
											{/if}
											{#if field.description}
												<p class="mt-1 text-xs text-surface-600">{field.description}</p>
											{/if}
										</div>
										{/if}
									{/each}
								</div>
							</div>
						{/if}
					</div>
				{/each}
			</div>
		</section>

	{/if}
</div>
