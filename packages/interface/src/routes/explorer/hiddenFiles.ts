import type { File } from "@sd/ts-client";
import { isHiddenFile, type HiddenFilter } from "./context";

export { isHiddenFile };
export type { HiddenFilter };

/** The name Finder would sort/display, including the extension. */
function displayName(file: File): string {
	return file.extension ? `${file.name}.${file.extension}` : file.name;
}

/**
 * Compare names the way macOS Finder does for its "Name" sort.
 * `localizedStandardCompare` is the documented Finder-like comparator; the web
 * equivalent is `localeCompare` with natural numeric ordering and case-folding.
 * A leading "." sorts before letters/digits, so dot files form a block at the top.
 */
export function finderNameCompare(a: File, b: File): number {
	return displayName(a).localeCompare(displayName(b), undefined, {
		numeric: true,
		sensitivity: "base",
	});
}

interface HiddenPreferenceOptions {
	hiddenFilter: HiddenFilter;
	/** Current sort field. Finder only blocks hidden items to the top for name sort. */
	sortBy: string;
	/** Whether folders are grouped before files (Finder "Keep folders on top"). */
	foldersFirst: boolean;
	/** Only process real directory listings; skip virtual/search/etc. lists. */
	enabled: boolean;
}

/**
 * Apply the user's hidden-files preference and Finder-style ordering to a
 * directory listing:
 *   - "hidden" filter keeps only dot-prefixed entries.
 *   - For a Name sort we re-sort client-side so dot entries group to the top
 *     (respecting Folders First), mirroring Finder's Name column.
 */
export function applyHiddenPreferences(
	files: File[],
	{ hiddenFilter, sortBy, foldersFirst, enabled }: HiddenPreferenceOptions,
): File[] {
	if (!enabled) return files;

	let result = files;

	if (hiddenFilter === "hidden") {
		result = result.filter((f) => isHiddenFile(f));
	}

	// Only reorder for Name sort; size/date/type sorts interleave hidden items.
	if (sortBy === "name") {
		result = [...result].sort((a, b) => {
			if (foldersFirst) {
				const aDir = a.kind === "Directory";
				const bDir = b.kind === "Directory";
				if (aDir !== bDir) return aDir ? -1 : 1;
			}
			return finderNameCompare(a, b);
		});
	}

	return result;
}
