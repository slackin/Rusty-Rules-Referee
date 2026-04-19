/** @param {string} s - UrT color-coded string like "^1Name^7rest" */
export function stripColors(s) {
	if (!s) return '';
	return s.replace(/\^\d/g, '');
}

/** Convert UrT color codes to HTML spans */
export function colorize(s) {
	if (!s) return '';
	const colors = {
		'^0': '#000000',
		'^1': '#ff0000',
		'^2': '#00ff00',
		'^3': '#ffff00',
		'^4': '#0066ff',
		'^5': '#00ffff',
		'^6': '#ff00ff',
		'^7': '#ffffff',
		'^8': '#ff8800',
		'^9': '#999999'
	};
	let html = '';
	let i = 0;
	while (i < s.length) {
		if (s[i] === '^' && i + 1 < s.length && colors[s.substring(i, i + 2)]) {
			const color = colors[s.substring(i, i + 2)];
			// Find next color code or end of string
			let end = i + 2;
			while (end < s.length) {
				if (s[end] === '^' && end + 1 < s.length && colors[s.substring(end, end + 2)]) break;
				end++;
			}
			const text = s.substring(i + 2, end).replace(/</g, '&lt;').replace(/>/g, '&gt;');
			html += `<span style="color:${color}">${text}</span>`;
			i = end;
		} else {
			html += s[i] === '<' ? '&lt;' : s[i] === '>' ? '&gt;' : s[i];
			i++;
		}
	}
	return html;
}

/** Format a timestamp to relative time */
export function timeAgo(ts) {
	if (!ts) return 'never';
	const now = Date.now();
	const then = new Date(ts).getTime();
	const diff = Math.floor((now - then) / 1000);
	if (diff < 60) return 'just now';
	if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
	if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
	if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
	return new Date(ts).toLocaleDateString();
}

/** Format seconds to human duration */
export function formatDuration(minutes) {
	if (!minutes || minutes <= 0) return 'permanent';
	if (minutes < 60) return `${minutes}m`;
	if (minutes < 1440) return `${Math.floor(minutes / 60)}h`;
	return `${Math.floor(minutes / 1440)}d`;
}

/** Weapon/item icon + color mapping by category */
export const gearIcons = {
	sidearm:  { icon: '🔫', color: 'text-blue-400', bg: 'bg-blue-500/10 border-blue-500/20' },
	rifle:    { icon: '🎯', color: 'text-orange-400', bg: 'bg-orange-500/10 border-orange-500/20' },
	sniper:   { icon: '🔭', color: 'text-purple-400', bg: 'bg-purple-500/10 border-purple-500/20' },
	shotgun:  { icon: '💥', color: 'text-amber-400', bg: 'bg-amber-500/10 border-amber-500/20' },
	smg:      { icon: '⚡', color: 'text-cyan-400', bg: 'bg-cyan-500/10 border-cyan-500/20' },
	launcher: { icon: '💣', color: 'text-red-400', bg: 'bg-red-500/10 border-red-500/20' },
	grenade:  { icon: '💥', color: 'text-red-400', bg: 'bg-red-500/10 border-red-500/20' },
	item:     { icon: '🛡️', color: 'text-green-400', bg: 'bg-green-500/10 border-green-500/20' },
};

/** Get icon info for a gear category */
export function getGearStyle(category) {
	return gearIcons[category] || gearIcons.item;
}

/** Format ping with color class */
export function pingColor(ping) {
	if (ping == null) return 'text-surface-500';
	if (ping <= 50) return 'text-green-400';
	if (ping <= 100) return 'text-yellow-400';
	if (ping <= 150) return 'text-orange-400';
	return 'text-red-400';
}

/** Team display info */
export function teamInfo(team) {
	const teams = {
		Red:       { label: 'Red', color: 'text-red-400', bg: 'bg-red-500/15 border-red-500/30' },
		Blue:      { label: 'Blue', color: 'text-blue-400', bg: 'bg-blue-500/15 border-blue-500/30' },
		Spectator: { label: 'Spec', color: 'text-surface-400', bg: 'bg-surface-500/15 border-surface-500/30' },
		Free:      { label: 'FFA', color: 'text-green-400', bg: 'bg-green-500/15 border-green-500/30' },
	};
	return teams[team] || { label: team || 'Unknown', color: 'text-surface-500', bg: 'bg-surface-500/15 border-surface-500/30' };
}
