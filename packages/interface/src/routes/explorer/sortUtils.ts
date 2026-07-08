import type { DirectorySortBy, File, MediaSortBy, SortDirection } from "@sd/ts-client";

export type SortBy = DirectorySortBy | MediaSortBy;
export type SortOrder = "asc" | "desc";

export function defaultSortOrder(sortBy: SortBy): SortOrder {
	if (sortBy === "modified" || sortBy === "size") {
		return "desc";
	}
	return "asc";
}

export function toSortDirection(order: SortOrder): SortDirection {
	return order === "asc" ? "Asc" : "Desc";
}

/** Client-side fallback when daemon hasn't been rebuilt yet. */
export function sortFiles(
	files: File[],
	sortBy: SortBy,
	sortOrder: SortOrder,
	foldersFirst: boolean,
): File[] {
	const direction = sortOrder === "asc" ? 1 : -1;
	const sorted = [...files];

	sorted.sort((a, b) => {
		if (foldersFirst && a.kind !== b.kind) {
			const aIsDir = a.kind === "Directory";
			const bIsDir = b.kind === "Directory";
			if (aIsDir !== bIsDir) {
				return aIsDir ? -1 : 1;
			}
		}

		let cmp = 0;
		switch (sortBy) {
			case "name":
				cmp = a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
				break;
			case "modified":
				cmp = a.modified_at.localeCompare(b.modified_at);
				break;
			case "size":
				cmp = a.size - b.size;
				break;
			case "type": {
				const aIsDir = a.kind === "Directory";
				const bIsDir = b.kind === "Directory";
				if (!foldersFirst && aIsDir !== bIsDir) {
					return aIsDir ? -1 : 1;
				}
				cmp = a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
				break;
			}
			default:
				cmp = 0;
		}

		return cmp * direction;
	});

	return sorted;
}