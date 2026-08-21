import { useMemo } from "react";
import type {
	ContentKind,
	File,
	FileSearchInput,
	FileSearchOutput,
	SearchScope as TSSearchScope,
	SdPath,
} from "../generated/types";
import { useNormalizedQuery } from "./useNormalizedQuery";

export type SearchScopeUI = "folder" | "location" | "library";

export interface UseSearchFilesOptions {
	/** Search query string (minimum 2 characters to search) */
	query: string;
	/** Search scope: "folder" (current path), "location", or "library" */
	scope: SearchScopeUI;
	/** Current path (required for "folder" scope) */
	currentPath?: SdPath | null;
	/** Location ID (required for "location" scope) */
	locationId?: string | null;
	/** Sort field: "Relevance", "Name", "Size", "ModifiedAt", "CreatedAt" */
	sortBy?: "Relevance" | "Name" | "Size" | "ModifiedAt" | "CreatedAt";
	/** Sort direction */
	sortDirection?: "Asc" | "Desc";
	/** Search mode */
	mode?: "Fast" | "Normal" | "Full";
	/** Search filters */
	filters?: {
		file_types?: string[] | null;
		tags?: { include: string[]; exclude: string[] } | null;
		date_range?: {
			field: "CreatedAt" | "ModifiedAt" | "AccessedAt" | "IndexedAt";
			start?: string | null;
			end?: string | null;
		} | null;
		size_range?: { min?: number | null; max?: number | null } | null;
		locations?: string[] | null;
		content_types?: ContentKind[] | null;
		include_hidden?: boolean | null;
		include_archived?: boolean | null;
	};
	/** Pagination limit */
	limit?: number;
	/** Whether query is enabled */
	enabled?: boolean;
}

export interface UseSearchFilesReturn {
	/** Search results */
	files: File[];
	/** Loading state */
	isLoading: boolean;
	/** Error state */
	error: Error | null;
}

/**
 * Shared search hook for fetching files via search.files query.
 * Platform-agnostic - can be used by both desktop and mobile.
 *
 * @example
 * ```tsx
 * const { files, isLoading } = useSearchFiles({
 *   query: "photos",
 *   scope: "library",
 *   sortBy: "Relevance",
 * });
 * ```
 */
export function useSearchFiles(
	options: UseSearchFilesOptions,
): UseSearchFilesReturn {
	const {
		query,
		scope,
		currentPath,
		locationId,
		sortBy = "Relevance",
		sortDirection = "Desc",
		mode = "Normal",
		filters,
		limit = 1000,
		enabled = true,
	} = options;

	// Map scope to TS SearchScope type
	const tsScope: TSSearchScope = useMemo(() => {
		if (scope === "folder" && currentPath) {
			return { Path: { path: currentPath } };
		}
		if (scope === "location" && locationId) {
			return { Location: { location_id: locationId } };
		}
		return "Library";
	}, [scope, currentPath, locationId]);

	// Build search input
	const searchInput: FileSearchInput = useMemo(
		() => ({
			query,
			scope: tsScope,
			mode,
			filters: {
				file_types: filters?.file_types ?? null,
				tags: filters?.tags ?? null,
				date_range: filters?.date_range
				? {
						field: filters.date_range.field,
						start: filters.date_range.start ?? null,
						end: filters.date_range.end ?? null,
					}
				: null,
				size_range: filters?.size_range
				? {
						min: filters.size_range.min ?? null,
						max: filters.size_range.max ?? null,
					}
				: null,
				locations: filters?.locations ?? null,
				content_types: filters?.content_types ?? null,
				include_hidden: filters?.include_hidden ?? null,
				include_archived: filters?.include_archived ?? null,
				at_risk: null,
				on_volumes: null,
				not_on_volumes: null,
				min_volume_count: null,
				max_volume_count: null,
			},
			sort: {
				field: sortBy,
				direction: sortDirection,
			},
			pagination: {
				limit,
				offset: 0,
			},
		}),
		[
			query,
			tsScope,
			mode,
			filters,
			sortBy,
			sortDirection,
			limit,
		],
	);

	// Execute search query
	const searchQuery = useNormalizedQuery<FileSearchInput, FileSearchOutput>({
		query: "search.files",
		input: searchInput,
		resourceType: "file",
		pathScope: scope === "folder" && currentPath ? currentPath : undefined,
		enabled: enabled && query.length >= 2,
	});

	const files =
		(searchQuery.data as FileSearchOutput | undefined)?.files || [];
	const isLoading = searchQuery.isLoading;
	const error = searchQuery.error;

	return { files, isLoading, error };
}
