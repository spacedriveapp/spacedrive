import type { DirectorySortBy, MediaSortBy, SortField } from "@sd/ts-client";

export type ExplorerSortBy = DirectorySortBy | MediaSortBy;
export type ExplorerViewMode = "grid" | "list" | "media" | "column";

export interface SortOption {
	value: ExplorerSortBy;
	label: string;
}

/**
 * The orders a view offers. Every view sorts by capture time now that the
 * listing reads it from the image facet; the media view adds birth time and
 * drops the type order, which means nothing in a roll.
 */
export function sortOptionsFor(viewMode: ExplorerViewMode): SortOption[] {
	if (viewMode === "media") {
		return [
			{ value: "datetaken", label: "Date Taken" },
			{ value: "modified", label: "Date Modified" },
			{ value: "created", label: "Date Created" },
			{ value: "name", label: "Name" },
			{ value: "size", label: "Size" },
		];
	}
	return [
		{ value: "name", label: "Name" },
		{ value: "modified", label: "Date Modified" },
		{ value: "datetaken", label: "Date Taken" },
		{ value: "size", label: "Size" },
		{ value: "type", label: "Type" },
	];
}

/**
 * The order to keep when a view changes: the current one when the new view
 * offers it, otherwise the view's first option, so a roll opens by capture
 * time and a folder falls back to its name.
 */
export function coerceSortBy(viewMode: ExplorerViewMode, sortBy: string | null | undefined): ExplorerSortBy {
	const options = sortOptionsFor(viewMode);
	const kept = options.find((option) => option.value === sortBy);
	return kept ? kept.value : options[0]!.value;
}

/** The search sort field an explorer order maps to. */
export function searchSortField(sortBy: string | null | undefined): SortField {
	switch (sortBy) {
		case "name":
			return "Name";
		case "size":
			return "Size";
		case "modified":
			return "ModifiedAt";
		case "created":
			return "CreatedAt";
		case "datetaken":
			return "CapturedAt";
		default:
			return "Relevance";
	}
}
