import { useMemo } from "react";
import { useQueries } from "@tanstack/react-query";
import type {
	ContentSearchHit,
	ContentSearchInput,
	ContentSearchOutput,
	SdPath,
	SpokenTime,
	TextKind,
} from "@sd/ts-client";
import { WIRE_METHODS, useLibraryQuery } from "@sd/ts-client";
import { useSpacedriveClient } from "../../../contexts/SpacedriveContext";
import { useSearchInput } from "./useExplorerFiles";

/** The text recordings and photos carry: what was said and what they show. */
const MEDIA_KINDS: TextKind[] = ["transcript", "scene", "caption"];

/** Hits asked of each scope. Several hits can be moments in one file. */
const HITS_PER_SCOPE = 50;

export interface ContentMoment {
	snippet: ContentSearchHit["snippet"];
	time: SpokenTime | null;
}

/** Every hit for one file, in the order the daemon ranked the file. */
export interface ContentFileHits {
	/** Absolute path on this device, for files.by_path. */
	path: string;
	name: string;
	/** The file's folder, relative to the scope searched. */
	folder: string;
	kinds: TextKind[];
	moments: ContentMoment[];
}

export interface ContentSearchResult {
	files: ContentFileHits[];
	isLoading: boolean;
	/** Files under the scopes whose text is not indexed yet. */
	pendingFiles: number;
}

function joinPath(root: string, relative: string): string {
	const separator = root.includes("\\") && !root.includes("/") ? "\\" : "/";
	const trimmed = root.endsWith(separator) ? root.slice(0, -1) : root;
	return trimmed + separator + relative.split("/").join(separator);
}

/**
 * Runs search.content beside the explorer's file search, over the same
 * scope, and groups its hits by file.
 *
 * The content index lives in each source's store on the device that owns
 * it, so a library-wide search asks every filesystem source this device
 * reads the contents of, one query each. A scope with no text index, or on
 * another device, answers with an error and contributes no hits.
 */
export function useContentSearch(): ContentSearchResult {
	const client = useSpacedriveClient();
	const searchInput = useSearchInput();
	const query = searchInput?.query.trim() ?? "";
	const isLibraryScope = searchInput?.scope === "Library";

	const { data: sources } = useLibraryQuery(
		{ type: "sources.list", input: { data_type: null } },
		{ enabled: !!query && isLibraryScope },
	);

	const scopes = useMemo<{ path: SdPath; root: string }[]>(() => {
		if (!searchInput || !query) return [];
		if (searchInput.scope !== "Library") {
			const path = searchInput.scope.Path.path;
			return "Physical" in path ? [{ path, root: path.Physical.path }] : [];
		}
		return (sources ?? []).flatMap((source) =>
			source.data_type === "filesystem" &&
			source.device_id === null &&
			source.attached &&
			source.root !== null &&
			source.settings?.identify_content !== false
				? [
						{
							path: {
								Physical: { device_slug: "local", path: source.root },
							},
							root: source.root,
						},
					]
				: [],
		);
	}, [searchInput, query, sources]);

	const libraryId = client.getCurrentLibraryId();
	return useQueries({
		queries: scopes.map((scope) => {
			const input: ContentSearchInput = {
				scope: scope.path,
				query,
				kinds: MEDIA_KINDS,
				max_results: HITS_PER_SCOPE,
				cursor: null,
			};
			return {
				queryKey: ["search.content", libraryId, input],
				queryFn: () =>
					client.execute<ContentSearchInput, ContentSearchOutput>(
						WIRE_METHODS.libraryQueries["search.content"],
						input,
					),
				enabled: !!libraryId,
				retry: false,
			};
		}),
		combine: (results) => {
			const byPath = new Map<string, ContentFileHits>();
			let pendingFiles = 0;
			results.forEach((result, index) => {
				if (!result.data) return;
				pendingFiles += result.data.pending_files;
				for (const hit of result.data.hits) {
					const path = joinPath(scopes[index].root, hit.path);
					let file = byPath.get(path);
					if (!file) {
						const slash = hit.path.lastIndexOf("/");
						file = {
							path,
							name: hit.path.slice(slash + 1),
							folder: slash > 0 ? hit.path.slice(0, slash) : "",
							kinds: [],
							moments: [],
						};
						byPath.set(path, file);
					}
					if (!file.kinds.includes(hit.kind)) file.kinds.push(hit.kind);
					file.moments.push({ snippet: hit.snippet, time: hit.time });
				}
			});
			const files = [...byPath.values()];
			for (const file of files) {
				file.moments.sort(
					(a, b) =>
						(a.time?.start_seconds ?? Infinity) -
						(b.time?.start_seconds ?? Infinity),
				);
			}
			return {
				files,
				isLoading: results.some((result) => result.isLoading),
				pendingFiles,
			};
		},
	});
}
