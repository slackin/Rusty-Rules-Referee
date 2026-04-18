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
