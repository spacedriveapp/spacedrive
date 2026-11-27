import { useQueries } from "@tanstack/react-query";
import type {
	SemanticSearchInput,
	SemanticSearchOutput,
	SemanticStatusInput,
	SemanticStatusOutput,
} from "@sd/ts-client";
import { WIRE_METHODS } from "@sd/ts-client";
import { useSpacedriveClient } from "../../../contexts/SpacedriveContext";
import { joinPath, useSearchScopes, type ContentFileHits } from "./useContentSearch";

/** Files asked of each scope. */
const HITS_PER_SCOPE = 30;

export interface SemanticAvailability {
	/** At least one scope can answer by meaning. */
	available: boolean;
	/** Why no scope can, from the daemon, when none can. */
	reason: string | null;
	/** Documents still waiting for embeddings across the scopes. */
	pendingDocuments: number;
	isLoading: boolean;
}

/**
 * Whether semantic search can run over the explorer's search scope. Each
 * source answers for itself, so the mode is offered when any source can.
 */
export function useSemanticAvailability(): SemanticAvailability {
	const client = useSpacedriveClient();
	const { scopes } = useSearchScopes({ always: true });
	const libraryId = client.getCurrentLibraryId();
	return useQueries({
		queries: scopes.map((scope) => {
			const input: SemanticStatusInput = { scope: scope.path };
			return {
				queryKey: ["search.semantic.status", libraryId, input],
				queryFn: () =>
					client.execute<SemanticStatusInput, SemanticStatusOutput>(
						WIRE_METHODS.libraryQueries["search.semantic.status"],
						input,
					),
				enabled: !!libraryId,
				retry: false,
				staleTime: 30_000,
			};
		}),
		combine: (results) => {
			const answered = results.flatMap((result) => (result.data ? [result.data] : []));
			const available = answered.some((status) => status.available);
			return {
				available,
				reason: available
					? null
					: (answered.find((status) => status.reason)?.reason ??
						(results.find((result) => result.error)?.error?.toString() ?? null)),
				pendingDocuments: answered.reduce(
					(sum, status) => sum + status.pending_documents,
					0,
				),
				isLoading: results.some((result) => result.isLoading),
			};
		},
	});
}

export interface SemanticSearchResult {
	/** Files in the order the daemon ranked them, best first. */
	files: ContentFileHits[];
	isLoading: boolean;
	pendingDocuments: number;
	/** Why a source's search failed, when one did. */
	error: string | null;
	/** Sources whose search failed; the others' files are still listed. */
	failedSources: number;
}

/**
 * Runs search.semantic over the explorer's search scope, one query per
 * source like content search, and keeps each source's ranking. Results of
 * several sources are interleaved by rank, since fused scores of two
 * sources are not comparable.
 */
export function useSemanticSearch(enabled: boolean): SemanticSearchResult {
	const client = useSpacedriveClient();
	const { scopes, query } = useSearchScopes({ always: false });
	const libraryId = client.getCurrentLibraryId();
	return useQueries({
		queries: scopes.map((scope) => {
			const input: SemanticSearchInput = {
				scope: scope.path,
				query,
				kinds: [],
				max_results: HITS_PER_SCOPE,
				cursor: null,
			};
			return {
				queryKey: ["search.semantic", libraryId, input],
				queryFn: () =>
					client.execute<SemanticSearchInput, SemanticSearchOutput>(
						WIRE_METHODS.libraryQueries["search.semantic"],
						input,
					),
				enabled: enabled && !!libraryId && !!query,
				retry: false,
			};
		}),
		combine: (results) => {
			const ranked: ContentFileHits[][] = results.map((result, index) =>
				(result.data?.hits ?? []).map((hit) => {
					const slash = hit.path.lastIndexOf("/");
					return {
						path: joinPath(scopes[index].root, hit.path),
						name: hit.path.slice(slash + 1),
						folder: slash > 0 ? hit.path.slice(0, slash) : "",
						kinds: [hit.kind],
						moments: [{ snippet: hit.snippet, time: hit.time }],
					};
				}),
			);
			const files: ContentFileHits[] = [];
			const longest = Math.max(0, ...ranked.map((list) => list.length));
			for (let rank = 0; rank < longest; rank++) {
				for (const list of ranked) {
					if (list[rank]) files.push(list[rank]);
				}
			}
			return {
				files,
				isLoading: results.some((result) => result.isLoading),
				pendingDocuments: results.reduce(
					(sum, result) => sum + (result.data?.pending_documents ?? 0),
					0,
				),
				error: results.find((result) => result.error)?.error?.toString() ?? null,
				failedSources: results.filter((result) => result.error).length,
			};
		},
	});
}
