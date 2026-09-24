// @ts-nocheck
import { useQuery, type UseQueryOptions, type UseQueryResult } from "@tanstack/react-query";
import { useSpacedriveClient } from "./useClient";
import type { LibraryPreview, LibraryValidate } from "../generated/types";
import { WIRE_METHODS } from "../generated/types";

/**
 * Whether and how a library action would run, over the input it takes.
 *
 * Validation is cheap, so it re-runs as the input changes: a dialog gates
 * its confirm button on the findings while options change, and dispatches
 * the identical input afterward.
 *
 * @example
 * ```tsx
 * const { data: validation } = useLibraryValidate({ type: 'files.merge', input });
 * const refused = validation?.findings.some((f) => f.severity === 'error');
 * ```
 */
export function useLibraryValidate<T extends LibraryValidate["type"]>(
	call: { type: T; input: Extract<LibraryValidate, { type: T }>["input"] },
	options?: Omit<
		UseQueryOptions<Extract<LibraryValidate, { type: T }>["output"]>,
		"queryKey" | "queryFn"
	>
): UseQueryResult<Extract<LibraryValidate, { type: T }>["output"]> {
	return usePreflight("validate", WIRE_METHODS.libraryValidates[call.type], call, options);
}

/**
 * What would exist after a library action, over the input it takes.
 *
 * A preview costs what the action touches and is advisory: the job applies
 * the same policy per leaf when it runs and reports where it diverged.
 *
 * @example
 * ```tsx
 * const { data: plan } = useLibraryPreview({ type: 'files.merge', input });
 * ```
 */
export function useLibraryPreview<T extends LibraryPreview["type"]>(
	call: { type: T; input: Extract<LibraryPreview, { type: T }>["input"] },
	options?: Omit<
		UseQueryOptions<Extract<LibraryPreview, { type: T }>["output"]>,
		"queryKey" | "queryFn"
	>
): UseQueryResult<Extract<LibraryPreview, { type: T }>["output"]> {
	return usePreflight("preview", WIRE_METHODS.libraryPreviews[call.type], call, options);
}

function usePreflight(kind, wireMethod, { type, input }, options) {
	const client = useSpacedriveClient();
	const libraryId = client.getCurrentLibraryId();

	return useQuery({
		queryKey: [kind, type, libraryId, input],
		queryFn: () => {
			if (!libraryId) {
				throw new Error("No library selected. Use client.switchToLibrary() first.");
			}
			return client.execute(wireMethod, input);
		},
		enabled: !!libraryId && (options?.enabled ?? true),
		...options,
	});
}
