import { useEffect, useMemo } from "react";
import type {
	DirectoryListingOutput,
	DirectorySortBy,
	File,
	FileSearchInput,
	FileSearchOutput,
	GetFilesByTagInput,
} from "@sd/ts-client";
import { useLibraryQuery } from "@sd/ts-client";
import {
	useNormalizedQuery,
	useSpacedriveClient,
} from "../../../contexts/SpacedriveContext";
import { useExplorer } from "../context";
import { searchSortField } from "../sortOptions";
import { isLapsedHandle, usePlanPreviewStore } from "./usePlanPreview";
import { useVirtualListing } from "./useVirtualListing";

export type FileSource =
	| "search"
	| "virtual"
	| "directory"
	| "recents"
	| "filtered"
	| "tag"
	| "collection"
	| "sourceMedia";

export interface ExplorerFilesResult {
	files: File[];
	isLoading: boolean;
	source: FileSource;
	/** True match count for a search, before pagination. */
	totalFound?: number;
	/** Facets over the whole match set, for the filter panel's counts. */
	facets?: FileSearchOutput['facets'];
}

/**
 * The search the explorer is running, exactly as it goes to the daemon, or
 * null outside search mode. Search mode means there is something to send: a
 * query long enough, or a filter that narrows the scope without one. Other
 * windows follow the same input through the navigation focus.
 */
export function useSearchInput(): FileSearchInput | null {
	const { mode, currentPath, sortBy, searchFilters, searchScope } =
		useExplorer();

	// The containing source for Source scope, resolved on the daemon with
	// alias normalization. Only fetched while that scope is active.
	const wantsSourceScope = mode.type === "search" && searchScope === "source";
	const { data: pathContext } = useLibraryQuery(
		{
			type: "paths.context",
			input: { path: currentPath! },
		},
		{ enabled: wantsSourceScope && !!currentPath },
	);
	const sourceScopePath = useMemo(() => {
		if (!wantsSourceScope || !pathContext?.source) return null;
		if (!currentPath || !("Physical" in currentPath)) return null;
		return {
			Physical: {
				device_slug: currentPath.Physical.device_slug,
				path: pathContext.source.root,
			},
		};
	}, [wantsSourceScope, pathContext, currentPath]);

	return useMemo<FileSearchInput | null>(() => {
		if (mode.type !== "search") return null;

		const { query } = mode;
		const scope = searchScope;

		// Source scope without a resolved source sends nothing rather than
		// silently widening to the library.
		if (scope === "source" && !sourceScopePath) return null;

		return {
			query,
			scope:
				scope === "folder" && currentPath
					? { Path: { path: currentPath } }
					: scope === "source" && sourceScopePath
						? { Path: { path: sourceScopePath } }
						: "Library",
			filters: searchFilters,
			mode: "Normal",
			sort: {
				field: searchSortField(sortBy),
				direction: "Desc",
			},
			pagination: {
				limit: 1000,
				offset: 0,
			},
		};
	}, [mode, searchScope, currentPath, sortBy, sourceScopePath, searchFilters]);
}

/**
 * Centralized hook for fetching files in the explorer.
 *
 * Handles file sources with priority:
 * 1. Filtered mode (e.g. redundancy views with pre-applied SearchFilters)
 * 2. Tag mode (when viewing files by tag)
 * 3. Search results (when in search mode)
 * 4. Recents (when in recents mode)
 * 5. Virtual listings (devices/volumes/sources)
 * 6. Directory listings (normal file browsing)
 */
export function useExplorerFiles(): ExplorerFilesResult {
	const explorer = useExplorer();
	const { mode, currentPath, sortBy, viewSettings, searchScope } = explorer;

	// Check for virtual listing first
	const { files: virtualFiles, isVirtualView } = useVirtualListing();

	// Check mode types
	const isSearchMode = mode.type === "search";
	const isRecentsMode = mode.type === "recents";
	const isFilteredMode = mode.type === "filtered";
	const isTagMode = mode.type === "tag";
	const isCollectionMode = mode.type === "collection";
	const isSourceMode = mode.type === "source";

	const searchQueryInput = useSearchInput();

	// Build filtered query input (pre-applied SearchFilters, e.g. redundancy views)
	const filteredQueryInput = useMemo<FileSearchInput | null>(() => {
		if (!isFilteredMode || mode.type !== "filtered") return null;

		const filteredSortField = (() => {
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
				field: filteredSortField,
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
			isSearchMode && searchScope === "folder" && currentPath
				? (currentPath as any)
				: undefined,
		enabled: !!searchQueryInput,
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

	// Tag query: files carrying the tag or any tag beneath it, so opening
	// `Camera` also shows what was tagged `Camera/Leica`
	const tagQueryInput = useMemo((): GetFilesByTagInput | null => {
		if (!isTagMode || mode.type !== "tag") return null;
		return {
			tag_id: mode.tagId,
			include_children: true,
		};
	}, [isTagMode, mode]);

	const tagQuery = useNormalizedQuery({
		query: "files.by_tag",
		input: tagQueryInput!,
		resourceType: "file",
		enabled: isTagMode && !!tagQueryInput,
	});

	// Collection query — entries identified at index time (screenshots, ...)
	const collectionQueryInput = useMemo(() => {
		if (!isCollectionMode || mode.type !== "collection") return null;
		return { slug: mode.slug, limit: null };
	}, [isCollectionMode, mode]);

	const collectionQuery = useNormalizedQuery({
		query: "files.collection_listing",
		input: collectionQueryInput!,
		resourceType: "file",
		enabled: isCollectionMode && !!collectionQueryInput,
	});

	// Source media query — archive records projected as Files
	const sourceQueryInput = useMemo(() => {
		if (!isSourceMode || mode.type !== "source") return null;
		return { source_id: mode.sourceId, limit: 2000, offset: 0 };
	}, [isSourceMode, mode]);

	const sourceQuery = useNormalizedQuery({
		query: "sources.media_listing",
		input: sourceQueryInput!,
		resourceType: "file",
		enabled: isSourceMode && !!sourceQueryInput,
	});

	// In preview mode the listing is the directory after the browsed plan.
	const overlay = usePlanPreviewStore((state) => state.preview?.plan.handle ?? null);
	const setOverlayRows = usePlanPreviewStore((state) => state.setRows);
	const rebuildPreview = usePlanPreviewStore((state) => state.rebuild);
	const client = useSpacedriveClient();

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
					overlay,
				}
			: null!,
		resourceType: "file",
		enabled:
			!!currentPath &&
			!isVirtualView &&
			!isSearchMode &&
			!isRecentsMode &&
			!isFilteredMode &&
			!isTagMode &&
			!isCollectionMode &&
			!isSourceMode,
		pathScope: currentPath ?? undefined,
	});

	// The rows the plan touches in this directory, for the views to mark.
	const overlayRows = (directoryQuery.data as DirectoryListingOutput | undefined)?.overlay;
	useEffect(() => {
		if (overlay) setOverlayRows(overlayRows ?? []);
	}, [overlay, overlayRows, setOverlayRows]);

	// A handle the daemon let lapse is rebuilt from the same input, and the
	// listing then re-runs with the new one.
	const listingError = directoryQuery.error;
	useEffect(() => {
		if (overlay && isLapsedHandle(listingError)) {
			void rebuildPreview(client);
		}
	}, [overlay, listingError, rebuildPreview, client]);

	// Priority: filtered > tag > recents > search > virtual > directory
	const source: FileSource = isFilteredMode
		? "filtered"
		: isSourceMode
			? "sourceMedia"
		: isCollectionMode
			? "collection"
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
		if (isSourceMode) {
			return (
				(sourceQuery.data as { files: File[] } | undefined)?.files ?? []
			);
		}
		if (isCollectionMode) {
			return (
				(collectionQuery.data as { files: File[] } | undefined)?.files ?? []
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
		isCollectionMode,
		isSourceMode,
		isRecentsMode,
		isSearchMode,
		isVirtualView,
		filteredQuery.data,
		collectionQuery.data,
		sourceQuery.data,
		tagQuery.data,
		recentsQuery.data,
		searchQuery.data,
		virtualFiles,
		directoryQuery.data,
	]);

	const isLoading = isFilteredMode
		? filteredQuery.isLoading
		: isSourceMode
			? sourceQuery.isLoading
		: isCollectionMode
			? collectionQuery.isLoading
		: isTagMode
			? tagQuery.isLoading
			: isRecentsMode
				? recentsQuery.isLoading
				: isSearchMode
					? searchQuery.isLoading
					: isVirtualView
						? false
						: directoryQuery.isLoading;

	const searchOutput = isSearchMode
		? (searchQuery.data as FileSearchOutput | undefined)
		: undefined;

	return {
		files,
		isLoading,
		source,
		totalFound:
			searchOutput?.total_found != null
				? Number(searchOutput.total_found)
				: undefined,
		facets: searchOutput?.facets,
	};
}
