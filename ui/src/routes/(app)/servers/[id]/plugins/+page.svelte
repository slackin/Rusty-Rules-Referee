<script>
	import { api } from '$lib/api.svelte.js';
	import { page } from '$app/stores';
	import { ChevronDown, ChevronRight, Plus, Trash2, Save, RotateCcw } from 'lucide-svelte';

	let serverId = $derived(Number($page.params.id));

	let catalog = $state([]);
	let plugins = $state([]);
	let originalJson = $state('');
	let expanded = $state({});

	let loading = $state(false);
	let saving = $state(false);
	let error = $state('');
	let message = $state('');
	let messageType = $state('');

	const eventTypes = [
		'EVT_GAME_ROUND_START',
		'EVT_GAME_ROUND_END',
		'EVT_GAME_MAP_CHANGE',
		'EVT_GAME_WARMUP',
		'EVT_GAME_EXIT',
	];

	let isDirty = $derived(JSON.stringify(plugins) !== originalJson);

	function defaultsFor(entry) {
		const obj = {};
		for (const f of entry.schema?.fields || []) {
			obj[f.key] = structuredClone(f.default);
		}
		return obj;
	}

	function mergePlugins(cat, overrides) {
		const byName = new Map((overrides || []).map(o => [o.name, o]));
		return cat.map(entry => {
			const ov = byName.get(entry.name);
			const baseSettings = defaultsFor(entry);
			const effective = { ...baseSettings, ...(ov?.settings || {}) };
			return {
				name: entry.name,
				enabled: ov ? ov.enabled : true,
				settings: effective,
				hasOverride: !!ov,
			};
		});
	}

	async function load() {
		loading = true;
		error = '';
		try {
			const r = await api.serverListPlugins(serverId);
			catalog = r.catalog || [];
			plugins = mergePlugins(catalog, r.overrides || []);
			originalJson = JSON.stringify(plugins);
		} catch (e) {
			error = e.message;
		} finally {
			loading = false;
		}
	}
	$effect(() => { load(); });

	function metaFor(name) {
		return catalog.find(e => e.name === name) || { label: name, description: '', schema: { fields: [] } };
	}

	function toggleExpand(name) {
		expanded[name] = !expanded[name];
		expanded = expanded;
	}

	function togglePlugin(idx) {
		plugins[idx].enabled = !plugins[idx].enabled;
		plugins = plugins;
	}

	function getPluginSetting(plugin, key, fallback) {
		const val = plugin.settings?.[key];
		return val !== undefined ? val : fallback;
	}

	function setPluginSetting(idx, key, value) {
		if (!plugins[idx].settings) plugins[idx].settings = {};
		plugins[idx].settings[key] = value;
		plugins = plugins;
	}

	function addListItem(idx, key) {
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

	function getKvEntries(plugin, key) {
		const obj = plugin.settings?.[key];
		if (!obj || typeof obj !== 'object' || Array.isArray(obj)) return [];
		return Object.entries(obj);
	}
	function addKvEntry(idx, key) {
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

	function getTaskList(plugin) {
		return Array.isArray(plugin.settings?.tasks) ? plugin.settings.tasks : [];
	}
	function addTask(idx) {
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

	function getMapConfigEntries(plugin) {
		const obj = plugin.settings?.map_configs;
		if (!obj || typeof obj !== 'object' || Array.isArray(obj)) return [];
		return Object.entries(obj);
	}
	function addMapConfig(idx) {
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

	function resetPlugin(idx) {
		const name = plugins[idx].name;
		const entry = metaFor(name);
		plugins[idx].settings = defaultsFor(entry);
		plugins[idx].enabled = true;
		plugins = plugins;
	}

	function resetAll() {
		plugins = JSON.parse(originalJson);
		message = '';
	}

	async function saveAll() {
		saving = true;
		message = '';
		error = '';
		const prev = JSON.parse(originalJson);
		const prevByName = new Map(prev.map(p => [p.name, p]));
		const changed = plugins.filter(p => {
			const before = prevByName.get(p.name);
			return !before || JSON.stringify(before) !== JSON.stringify(p);
		});
		try {
			for (const p of changed) {
				await api.serverUpdatePlugin(serverId, p.name, {
					enabled: p.enabled,
					settings: p.settings,
				});
			}
			message = `Saved ${changed.length} plugin${changed.length === 1 ? '' : 's'}. Client will pick up changes on next heartbeat.`;
			messageType = 'success';
			originalJson = JSON.stringify(plugins);
		} catch (e) {
			error = e.message;
		} finally {
			saving = false;
		}
	}
</script>

<div class="space-y-4 animate-fade-in">
	<div class="flex items-start justify-between gap-4">
		<div>
			<h2 class="text-xl font-semibold">Plugins</h2>
			<p class="mt-1 text-sm text-surface-500">
				Configure plugins for this server. Changes are pushed to the server's bot on
				its next heartbeat; the bot will restart to apply new plugin settings.
			</p>
		</div>
		<div class="flex gap-2 flex-shrink-0">
			<button class="btn-secondary btn-sm" onclick={resetAll} disabled={!isDirty || saving}>
				<RotateCcw class="h-3.5 w-3.5" /> Reset
			</button>
			<button class="btn-primary btn-sm" onclick={saveAll} disabled={!isDirty || saving}>
				{#if saving}
					<div class="h-3.5 w-3.5 animate-spin rounded-full border-2 border-white/30 border-t-white"></div>
				{:else}
					<Save class="h-3.5 w-3.5" />
				{/if}
				Save
			</button>
		</div>
	</div>

	{#if error}
		<div class="rounded-lg bg-red-500/10 px-4 py-3 text-sm text-red-400 ring-1 ring-red-500/20">{error}</div>
	{/if}
	{#if message}
		<div class="rounded-lg px-4 py-3 text-sm {messageType === 'success' ? 'bg-emerald-500/10 text-emerald-400 ring-1 ring-emerald-500/20' : 'bg-surface-800 text-surface-300'}">{message}</div>
	{/if}

	{#if loading && plugins.length === 0}
		<div class="flex items-center justify-center py-20">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-accent/20 border-t-accent"></div>
		</div>
	{:else}
		<div class="space-y-2">
			{#each plugins as plugin, idx}
				{@const meta = metaFor(plugin.name)}
				{@const fields = meta.schema?.fields || []}
				{@const hasSettings = fields.length > 0}
				{@const isExpanded = expanded[plugin.name]}
				<div class="card overflow-hidden">
					<div class="flex items-center gap-3 px-5 py-3.5">
						{#if hasSettings}
							<button type="button" class="p-0.5 text-surface-500 hover:text-surface-300" onclick={() => toggleExpand(plugin.name)}>
								{#if isExpanded}<ChevronDown class="h-4 w-4" />{:else}<ChevronRight class="h-4 w-4" />{/if}
							</button>
						{:else}
							<span class="w-5"></span>
						{/if}

						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<!-- svelte-ignore a11y_click_events_have_key_events -->
						<div class="flex-1 min-w-0" class:cursor-pointer={hasSettings} onclick={() => hasSettings && toggleExpand(plugin.name)}>
							<div class="flex items-center gap-2">
								<span class="text-sm font-medium text-surface-200">{meta.label}</span>
								<span class="font-mono text-xs text-surface-600">{plugin.name}</span>
								{#if plugin.hasOverride}
									<span class="rounded bg-accent/10 px-1.5 py-0.5 text-[10px] font-medium text-accent">override</span>
								{/if}
							</div>
							{#if meta.description}
								<p class="text-xs text-surface-500 truncate">{meta.description}</p>
							{/if}
						</div>

						<button
							type="button"
							aria-label="Toggle {meta.label} plugin"
							class="relative h-5 w-9 flex-shrink-0 rounded-full transition-colors {plugin.enabled ? 'bg-accent' : 'bg-surface-700'}"
							onclick={() => togglePlugin(idx)}
						>
							<span class="absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-white shadow transition-transform {plugin.enabled ? 'translate-x-4' : ''}"></span>
						</button>
					</div>

					{#if hasSettings && isExpanded}
						<div class="border-t border-surface-800 bg-surface-950/30 px-5 py-4">
							<div class="grid gap-4 sm:grid-cols-2">
								{#each fields as field}
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
												{#each entries as [k, v]}
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
												{#each entries as [k, v]}
													{@const dur = typeof v === 'object' && v ? (v.duration ?? '') : ''}
													{@const reason = typeof v === 'object' && v ? (v.reason ?? '') : (typeof v === 'string' ? v : '')}
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
													{#each field.options || [] as opt}
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

							<div class="mt-4 flex justify-end border-t border-surface-800 pt-3">
								<button type="button" class="text-xs text-surface-500 hover:text-surface-300" onclick={() => resetPlugin(idx)}>
									Reset to defaults
								</button>
							</div>
						</div>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>
