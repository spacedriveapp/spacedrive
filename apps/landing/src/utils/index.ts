/**
 * Accessor for the browser's `window` object, so that `window` is
 * not access during SSG.
 */
export function getWindow(): (Window & typeof globalThis) | null {
	return typeof window !== 'undefined' ? window : null;
}

// eslint-disable-next-line no-useless-escape
const FILE_NAME_REGEX = /^.*[\\\/]/;

/**
 * Extracts the file name including its extension from a file path
 */
export function filename(path: string) {
	return path.replace(FILE_NAME_REGEX, '');
}

/**
 * Takes the result of `import.meta.globEager` and returns an object
 * with the keys being the file names and the values being the imported file.
 *
 * Does not work with directories.
 */
export function resolveFilesGlob(files: Record<string, any>): Record<string, string> {
	return Object.entries(files).reduce(
		(acc, [name, val]) => ({ ...acc, [filename(name)]: val.default }),
		{}
	);
}
