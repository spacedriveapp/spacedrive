/**
 * Accessor for the browser's `window` object, so that `window` is
 * not access during SSG.
 */
export function getWindow(): (Window & typeof globalThis) | null {
	return typeof window !== 'undefined' ? window : null;
}

export function toTitleCase(str: string) {
	return str
		.toLowerCase()
		.replace(/(?:^|[\s-/])\w/g, function (match) {
			return match.toUpperCase();
		})
		.replaceAll('-', ' ');
}
