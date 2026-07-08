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

export function sortDirectionLabel(sortBy: SortBy, sortOrder: SortOrder): string {
	switch (sortBy) {
		case "name":
		case "type":
			return sortOrder === "asc" ? "A–Z" : "Z–A";
		case "modified":
		case "created":
		case "datetaken":
			return sortOrder === "asc" ? "Oldest first" : "Newest first";
		case "size":
			return sortOrder === "asc" ? "Smallest first" : "Largest first";
		default:
			return sortOrder === "asc" ? "Ascending" : "Descending";
	}
}

/** Locale-aware sort. Used for virtual lists and to re-apply order after cache updates. */
export function sortFiles(
	files: File[],
	sortBy: SortBy,
	sortOrder: SortOrder,
	foldersFirst: boolean,
): File[] {
	const direction = sortOrder === "asc" ? 1 : -1;
	const sorted = [...files];

	// natural + case-insensitive: "App" and "app" group together; file2 before file10
	const byName = (a: string, b: string) =>
		a.localeCompare(b, undefined, { sensitivity: "base", numeric: true });

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
				cmp = byName(a.name, b.name);
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
				cmp = byName(a.name, b.name);
				break;
			}
			default:
				cmp = 0;
		}

		return cmp * direction;
	});

	return sorted;
}