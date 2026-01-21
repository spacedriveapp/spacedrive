import { useMemo } from "react";
import type { File, SdPath } from "@sd/ts-client";
import { useNormalizedQuery } from "../../../client";
import { useVirtualListing } from "./useVirtualListing";

export type FileSource = "virtual" | "directory";

export interface ExplorerFilesResult {
	files: File[];
	isLoading: boolean;
	source: FileSource;
}

/**
 * Centralized hook for fetching files in the mobile explorer.
 *
 * Handles two file sources with priority:
 * 1. Virtual listings (devices/volumes/locations)
 * 2. Directory listings (normal file browsing)
 */
export function useExplorerFiles(
	params:
		| { type: "path"; path: string }
		| { type: "view"; view: string; id?: string }
		| undefined,
): ExplorerFilesResult {
	// Check for virtual listing first
	const { files: virtualFiles, isVirtualView, isLoading: virtualLoading } = useVirtualListing(params);

	// Parse path for directory listing
	const currentPath: SdPath | null = useMemo(() => {
		if (params?.type === "path") {
			try {
				return JSON.parse(params.path) as SdPath;
			} catch (e) {
				console.error("[useExplorerFiles] Failed to parse path:", e);
				return null;
			}
		}
		return null;
	}, [params]);

	// Directory query
	const directoryQuery = useNormalizedQuery({
		wireMethod: "query:files.directory_listing",
		input: currentPath
			? {
					path: currentPath,
					limit: null,
					include_hidden: false,
					sort_by: "name", // Default to name sorting
					folders_first: true,
				}
			: null!,
		resourceType: "file",
		enabled: !!currentPath && !isVirtualView,
		pathScope: currentPath ?? undefined,
	});

	console.log("[useExplorerFiles] Query state:", {
		isVirtualView,
		hasPath: !!currentPath,
		enabled: !!currentPath && !isVirtualView,
		isLoading: directoryQuery.isLoading,
		hasData: !!directoryQuery.data,
		fileCount: (directoryQuery.data as any)?.files?.length || 0,
	});

	// Determine source and files with priority: virtual > directory
	const source: FileSource = isVirtualView ? "virtual" : "directory";

	const files = useMemo(() => {
		if (isVirtualView) {
			return virtualFiles || [];
		}
		return (directoryQuery.data as any)?.files || [];
	}, [isVirtualView, virtualFiles, directoryQuery.data]);

	return {
		files,
		isLoading: isVirtualView ? virtualLoading : directoryQuery.isLoading,
		source,
	};
}
