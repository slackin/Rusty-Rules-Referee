// Round-trip-safe parser / serializer for Urban Terror server.cfg files.
//
// Philosophy: keep the original file unchanged except for cvars the user
// actually edited. Unknown cvars, comments, blank lines, binds, and
// exec/vstr commands all pass through verbatim.
//
// Line model:
//   { kind, raw, key?, value?, quoted?, trailing? }
// where kind ∈ 'set' | 'comment' | 'blank' | 'other'.

const SET_RE = /^(\s*)(set|seta|sets|setu)(\s+)("?)([^\s"]+)\4(\s+)(.*?)(\s*\/\/.*)?$/i;

/**
 * Parse raw server.cfg text into a line array + cvar index.
 * @param {string} text
 * @returns {{ lines: Array, cvars: Record<string, {value:string, line:number}>, original: string }}
 */
export function parseCfg(text) {
	const src = text ?? '';
	const lines = [];
	const cvars = {};

	const rawLines = src.split(/\r?\n/);
	for (let i = 0; i < rawLines.length; i++) {
		const raw = rawLines[i];
		const trimmed = raw.trim();

		if (trimmed === '') {
			lines.push({ kind: 'blank', raw });
			continue;
		}
		if (trimmed.startsWith('//') || trimmed.startsWith('#')) {
			lines.push({ kind: 'comment', raw });
			continue;
		}

		const m = raw.match(SET_RE);
		if (m) {
			const [, indent, setw, gap1, _q, key, gap2, rest, trailing] = m;
			const { value, quoted } = extractValue(rest);
			lines.push({
				kind: 'set',
				raw,
				indent,
				setw,
				gap1,
				gap2,
				key,
				value,
				quoted,
				trailing: trailing || '',
			});
			// Last write wins — matches engine behavior.
			cvars[key] = { value, line: lines.length - 1 };
			continue;
		}

		// Everything else (bind, exec, vstr, say, etc.) is preserved verbatim.
		lines.push({ kind: 'other', raw });
	}

	return { lines, cvars, original: src };
}

/**
 * Parse the argument of a `set <key>` line, stripping surrounding
 * quotes but preserving inner content.
 */
function extractValue(rest) {
	const r = rest ?? '';
	const trimmed = r.trimEnd();
	if (trimmed.length >= 2 && trimmed[0] === '"' && trimmed.endsWith('"')) {
		return { value: trimmed.slice(1, -1), quoted: true };
	}
	return { value: trimmed, quoted: false };
}

/**
 * Build the replacement text for a single set line, preserving the
 * original `set|seta|sets|setu` keyword, indentation, and trailing
 * comment (if any). Always requotes the new value so values with
 * spaces / semicolons survive.
 */
function renderSetLine(line, newValue) {
	const indent = line.indent ?? '';
	const setw = line.setw ?? 'set';
	const gap1 = line.gap1 ?? ' ';
	const gap2 = line.gap2 ?? ' ';
	const trailing = line.trailing ?? '';
	return `${indent}${setw}${gap1}${line.key}${gap2}"${newValue}"${trailing}`;
}

/**
 * Build a brand-new `set <key> "value"` line for a cvar that wasn't
 * present in the original file.
 */
export function renderNewSetLine(key, value) {
	return `seta ${key} "${value}"`;
}

/**
 * Serialize a parsed cfg with the given cvar overrides applied.
 *
 * @param {ReturnType<typeof parseCfg>} parsed
 * @param {Record<string,string>} desired     — full set of desired cvar
 *                                              values (including unchanged).
 *                                              Missing keys in `desired`
 *                                              that ARE present in the file
 *                                              will be left alone.
 * @param {Array<{key:string,value:string}>} [newCvars] — cvars to append
 *                                              that didn't exist before.
 * @returns {string}
 */
export function serializeCfg(parsed, desired, newCvars = []) {
	const outLines = parsed.lines.map((ln) => {
		if (ln.kind !== 'set') return ln.raw;
		const want = desired[ln.key];
		if (want === undefined || want === null) return ln.raw;
		const current = ln.value ?? '';
		if (String(want) === String(current)) return ln.raw; // unchanged
		return renderSetLine(ln, String(want));
	});

	// Preserve whether the original ended with a newline.
	const endedWithNewline = (parsed.original ?? '').endsWith('\n');
	let out = outLines.join('\n');

	if (newCvars.length > 0) {
		// Ensure a blank line separator.
		if (out.length > 0 && !out.endsWith('\n')) out += '\n';
		if (!out.endsWith('\n\n')) out += '\n';
		out += '// --- Added by R3 admin UI ---\n';
		for (const { key, value } of newCvars) {
			out += renderNewSetLine(key, String(value)) + '\n';
		}
		return out;
	}

	if (endedWithNewline && !out.endsWith('\n')) out += '\n';
	return out;
}

/**
 * Diff helper for tests / "what will change" previews.
 * Returns array of { key, from, to } for cvars whose value differs
 * from `parsed`. Ignores cvars not present in parsed (those go in
 * newCvars).
 */
export function diffCvars(parsed, desired) {
	const out = [];
	for (const [key, val] of Object.entries(desired)) {
		const existing = parsed.cvars[key];
		if (existing && String(existing.value) !== String(val)) {
			out.push({ key, from: existing.value, to: String(val) });
		}
	}
	return out;
}
