import { useMemo } from "react";
import type { DirectorySortBy, File, FileSearchInput, FileSearchOutput } from "@sd/ts-client";
import { useNormalizedQuery } from "../../../contexts/SpacedriveContext";
import { useExplorer } from "../context";
import type { SearchScope } from "../context";
import { useVirtualListing } from "./useVirtualListing";

export type FileSource = "search" | "virtual" | "directory" | "recents";

export interface ExplorerFilesResult {
	files: File[];
	isLoading: boolean;
	source: FileSource;
}

/**
 * Centralized hook for fetching files in the explorer.
 *
 * Handles three file sources with priority:
 * 1. Search results (when in search mode)
 * 2. Virtual listings (devices/volumes/locations)
 * 3. Directory listings (normal file browsing)
 */
export function useExplorerFiles(): ExplorerFilesResult {
	const explorer = useExplorer();
	const { mode, currentPath, sortBy, viewSettings } = explorer;

	// Check for virtual listing first
	const { files: virtualFiles, isVirtualView } = useVirtualListing();

	// Check for search mode
	const isSearchMode = mode.type === "search";
	const isRecentsMode = mode.type === "recents";

	// Build search query input
	const searchQueryInput = useMemo<FileSearchInput | null>(() => {
		if (!isSearchMode) return null;

		const searchMode = mode;
		if (searchMode.type !== "search") return null;

		const { query, scope } = searchMode;

		// Map explorer sortBy to search SortField
		const searchSortField = (() => {
			if (!sortBy) return "Relevance" as const;
			const sortMap: Record<string, "Relevance" | "Name" | "Size" | "ModifiedAt" | "CreatedAt"> = {
				name: "Name",
				size: "Size",
				modified: "ModifiedAt",
				type: "Relevance",
			};
			return sortMap[sortBy] || "Relevance";
		})();

		return {
			query,
			scope:
				scope === "folder" && currentPath
					? { Path: { path: currentPath } }
					: "Library",
			filters: {
				file_types: null,
				tags: null,
				date_range: null,
				size_range: null,
				locations: null,
				content_types: null,
				include_hidden: null,
				include_archived: null,
			},
			mode: "Normal",
			sort: {
				field: searchSortField,
				direction: "Desc",
			},
			pagination: {
				limit: 1000,
				offset: 0,
			},
		};
	}, [isSearchMode, mode, currentPath, sortBy]);

	// Build recents query input
	const recentsQueryInput = useMemo<FileSearchInput | null>(() => {
		if (!isRecentsMode) return null;

		return {
			query: "", // Empty query to match all files
			scope: "Library",
			filters: {
				file_types: null,
				tags: null,
				date_range: null,
				size_range: null,
				locations: null,
				content_types: null,
				include_hidden: null,
				include_archived: null,
			},
			mode: "Fast", // Fast mode since we're just sorting by indexed_at
			sort: {
				field: "IndexedAt", // Sort by when files were indexed
				direction: "Desc", // Most recent first
			},
			pagination: {
				limit: 100, // Reasonable limit for recents screen
				offset: 0,
			},
		};
	}, [isRecentsMode]);

	// Search query
	const searchQuery = useNormalizedQuery<FileSearchInput, FileSearchOutput>({
		wireMethod: "query:search.files",
		input: searchQueryInput!,
		resourceType: "file",
		pathScope:
			isSearchMode && mode.type === "search" && mode.scope === "folder" && currentPath
				? (currentPath as any)
				: undefined,
		enabled: isSearchMode && !!searchQueryInput && searchQueryInput.query.length >= 2,
	});

	// Recents query
	const recentsQuery = useNormalizedQuery<FileSearchInput, FileSearchOutput>({
		wireMethod: "query:search.files",
		input: recentsQueryInput!,
		resourceType: "file",
		enabled: isRecentsMode && !!recentsQueryInput,
	});

	// Directory query
	const directoryQuery = useNormalizedQuery({
		wireMethod: "query:files.directory_listing",
		input: currentPath
			? {
					path: currentPath,
					limit: null,
					include_hidden: false,
					sort_by: sortBy as DirectorySortBy,
					folders_first: viewSettings.foldersFirst,
				}
			: null!,
		resourceType: "file",
		enabled: !!currentPath && !isVirtualView && !isSearchMode && !isRecentsMode,
		pathScope: currentPath ?? undefined,
	});

	// Determine source and files with priority: recents > search > virtual > directory
	const source: FileSource = isRecentsMode
		? "recents"
		: isSearchMode
			? "search"
			: isVirtualView
				? "virtual"
				: "directory";

	const files = useMemo(() => {
		if (isRecentsMode) {
			return (recentsQuery.data as FileSearchOutput | undefined)?.files || [];
		}
		if (isSearchMode) {
			return (searchQuery.data as FileSearchOutput | undefined)?.files || [];
		}
		if (isVirtualView) {
			return virtualFiles || [];
		}
		return (directoryQuery.data as any)?.files || [];
	}, [isRecentsMode, isSearchMode, isVirtualView, recentsQuery.data, searchQuery.data, virtualFiles, directoryQuery.data]);

	const isLoading = isRecentsMode
		? recentsQuery.isLoading
		: isSearchMode
			? searchQuery.isLoading
			: isVirtualView
				? false
				: directoryQuery.isLoading;

	return { files, isLoading, source };
}
