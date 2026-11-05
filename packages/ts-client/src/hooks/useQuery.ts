// @ts-nocheck
import { useQuery, type UseQueryOptions, type UseQueryResult } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useSpacedriveClient } from "./useClient";
import type { CoreQuery, LibraryQuery } from "../generated/types";
import { WIRE_METHODS } from "../generated/types";

/**
 * Type-safe hook for core-scoped queries
 *
 * @example
 * ```tsx
 * function LibraryList() {
 *   const { data: libraries } = useCoreQuery({
 *     type: 'libraries.list',
 *     input: {}
 *   });
 *
 *   return <div>{libraries?.map(lib => lib.name)}</div>;
 * }
 * ```
 */
export function useCoreQuery<T extends CoreQuery["type"]>(
	query: { type: T; input: Extract<CoreQuery, { type: T }>["input"] },
	options?: Omit<
		UseQueryOptions<Extract<CoreQuery, { type: T }>["output"]>,
		"queryKey" | "queryFn"
	>
): UseQueryResult<Extract<CoreQuery, { type: T }>["output"]> {
	const client = useSpacedriveClient();
	const wireMethod = WIRE_METHODS.coreQueries[query.type];  // ← Auto-generated!

	return useQuery({
		queryKey: [query.type, query.input],
		queryFn: () => client.execute(wireMethod, query.input),
		...options,
	}) as UseQueryResult<Extract<CoreQuery, { type: T }>["output"]>;
}

/**
 * Type-safe hook for library-scoped queries
 *
 * Automatically uses the current library ID from the client
 *
 * @example
 * ```tsx
 * function FileExplorer() {
 *   const { data: files } = useLibraryQuery({
 *     type: 'files.directory_listing',
 *     input: { path: '/' }
 *   });
 *
 *   const { data: jobs } = useLibraryQuery({
 *     type: 'jobs.list',
 *     input: {}
 *   });
 *
 *   return <div>{files?.entries.map(file => file.name)}</div>;
 * }
 * ```
 */
export function useLibraryQuery<T extends LibraryQuery["type"]>(
	query: { type: T; input: Extract<LibraryQuery, { type: T }>["input"] },
	options?: Omit<
		UseQueryOptions<Extract<LibraryQuery, { type: T }>["output"]>,
		"queryKey" | "queryFn"
	>
): UseQueryResult<Extract<LibraryQuery, { type: T }>["output"]> {
	const client = useSpacedriveClient();
	const wireMethod = WIRE_METHODS.libraryQueries[query.type];  // ← Auto-generated!
	// A page that mounts before the app restores its library would otherwise
	// keep a disabled query until something else re-renders it.
	const [libraryId, setLibraryId] = useState<string | null>(() =>
		client.getCurrentLibraryId(),
	);
	useEffect(() => {
		setLibraryId(client.getCurrentLibraryId());
		client.on("library-changed", setLibraryId);
		return () => {
			client.off("library-changed", setLibraryId);
		};
	}, [client]);

	return useQuery({
		queryKey: [query.type, libraryId, query.input],
		queryFn: () => {
			if (!libraryId) {
				throw new Error("No library selected. Use client.switchToLibrary() first.");
			}

			// Client.execute() automatically adds library_id to the request
			// as a sibling field (not inside payload)
			return client.execute(wireMethod, query.input);
		},
		enabled: !!libraryId && (options?.enabled ?? true),
		...options,
	}) as UseQueryResult<Extract<LibraryQuery, { type: T }>["output"]>;
}
