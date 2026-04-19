import { useMemo } from "react";
import type { DirectorySortBy, File, FileSearchInput, FileSearchOutput } from "@sd/ts-client";
import { useNormalizedQuery } from "../../../contexts/SpacedriveContext";
import { useExplorer } from "../context";
import { useVirtualListing } from "./useVirtualListing";

export type FileSource =
	| "search"
	| "virtual"
	| "directory"
	| "recents"
	| "filtered"
	| "tag";

export interface ExplorerFilesResult {
	files: File[];
	isLoading: boolean;
	source: FileSource;
}

/**
 * Centralized hook for fetching files in the explorer.
 *
 * Handles file sources with priority:
 * 1. Filtered mode (e.g. redundancy views with pre-applied SearchFilters)
 * 2. Tag mode (when viewing files by tag)
 * 3. Search results (when in search mode)
 * 4. Recents (when in recents mode)
 * 5. Virtual listings (devices/volumes/locations)
 * 6. Directory listings (normal file browsing)
 */
export function useExplorerFiles(): ExplorerFilesResult {
	const explorer = useExplorer();
	const { mode, currentPath, sortBy, viewSettings } = explorer;

	// Check for virtual listing first
	const { files: virtualFiles, isVirtualView } = useVirtualListing();

	// Check mode types
	const isSearchMode = mode.type === "search";
	const isRecentsMode = mode.type === "recents";
	const isFilteredMode = mode.type === "filtered";
	const isTagMode = mode.type === "tag";

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
				at_risk: null,
				on_volumes: null,
				not_on_volumes: null,
				min_volume_count: null,
				max_volume_count: null,
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

	// Build filtered query input (pre-applied SearchFilters, e.g. redundancy views)
	const filteredQueryInput = useMemo<FileSearchInput | null>(() => {
		if (!isFilteredMode || mode.type !== "filtered") return null;

		const searchSortField = (() => {
			if (!sortBy) return "Size" as const;
			const sortMap: Record<
				string,
				"Relevance" | "Name" | "Size" | "ModifiedAt" | "CreatedAt"
			> = {
				name: "Name",
				size: "Size",
				modified: "ModifiedAt",
				type: "Size",
			};
			return sortMap[sortBy] || "Size";
		})();

		return {
			query: "",
			scope: "Library",
			filters: mode.filters,
			mode: "Fast",
			sort: {
				field: searchSortField,
				direction: "Desc",
			},
			pagination: {
				limit: 1000,
				offset: 0,
			},
		};
	}, [isFilteredMode, mode, sortBy]);

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
				at_risk: null,
				on_volumes: null,
				not_on_volumes: null,
				min_volume_count: null,
				max_volume_count: null,
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
		query: "search.files",
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
		query: "search.files",
		input: recentsQueryInput!,
		resourceType: "file",
		enabled: isRecentsMode && !!recentsQueryInput,
	});

	// Filtered query (pre-applied SearchFilters)
	const filteredQuery = useNormalizedQuery<FileSearchInput, FileSearchOutput>({
		query: "search.files",
		input: filteredQueryInput!,
		resourceType: "file",
		enabled: isFilteredMode && !!filteredQueryInput,
	});

	// Tag query — fetches files tagged with a specific tag
	const tagQueryInput = useMemo(() => {
		if (!isTagMode || mode.type !== "tag") return null;
		return {
			tag_id: mode.tagId,
			include_children: false,
			min_confidence: 0.0,
		};
	}, [isTagMode, mode]);

	const tagQuery = useNormalizedQuery({
		query: "files.by_tag",
		input: tagQueryInput!,
		resourceType: "file",
		enabled: isTagMode && !!tagQueryInput,
	});

	// Directory query
	const directoryQuery = useNormalizedQuery({
		query: "files.directory_listing",
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
		enabled:
			!!currentPath &&
			!isVirtualView &&
			!isSearchMode &&
			!isRecentsMode &&
			!isFilteredMode &&
			!isTagMode,
		pathScope: currentPath ?? undefined,
	});

	// Priority: filtered > tag > recents > search > virtual > directory
	const source: FileSource = isFilteredMode
		? "filtered"
		: isTagMode
			? "tag"
			: isRecentsMode
				? "recents"
				: isSearchMode
					? "search"
					: isVirtualView
						? "virtual"
						: "directory";

	const files = useMemo(() => {
		if (isFilteredMode) {
			return (
				(filteredQuery.data as FileSearchOutput | undefined)?.files || []
			);
		}
		if (isTagMode) {
			return (tagQuery.data as { files: File[] } | undefined)?.files ?? [];
		}
		if (isRecentsMode) {
			return (recentsQuery.data as FileSearchOutput | undefined)?.files || [];
		}
		if (isSearchMode) {
			return (searchQuery.data as FileSearchOutput | undefined)?.files || [];
		}
		if (isVirtualView) {
			return virtualFiles || [];
		}
		return (directoryQuery.data as { files: File[] } | undefined)?.files ?? [];
	}, [
		isFilteredMode,
		isTagMode,
		isRecentsMode,
		isSearchMode,
		isVirtualView,
		filteredQuery.data,
		tagQuery.data,
		recentsQuery.data,
		searchQuery.data,
		virtualFiles,
		directoryQuery.data,
	]);

	const isLoading = isFilteredMode
		? filteredQuery.isLoading
		: isTagMode
			? tagQuery.isLoading
			: isRecentsMode
				? recentsQuery.isLoading
				: isSearchMode
					? searchQuery.isLoading
					: isVirtualView
						? false
						: directoryQuery.isLoading;

	return { files, isLoading, source };
}
