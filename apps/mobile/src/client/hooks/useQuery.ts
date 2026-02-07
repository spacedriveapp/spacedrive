import {
	useQuery,
	useMutation,
	UseQueryOptions,
	UseMutationOptions,
} from "@tanstack/react-query";
import { useSpacedriveClient } from "./useClient";
import type { SpacedriveClient } from "../SpacedriveClient";

// Cast hook result to mobile's client type
function useMobileClient(): SpacedriveClient {
	return useSpacedriveClient() as unknown as SpacedriveClient;
}

/**
 * Hook for executing core-level queries (no library context).
 */
export function useCoreQuery<T = unknown>(
	method: string,
	input: unknown = {},
	options?: Omit<UseQueryOptions<T, Error>, "queryKey" | "queryFn">,
) {
	const client = useMobileClient();

	return useQuery<T, Error>({
		queryKey: ["core", method, input],
		queryFn: () => client.coreQuery<T>(method, input),
		...options,
	});
}

/**
 * Hook for executing library-level queries.
 * Automatically uses the current library context.
 */
export function useLibraryQuery<T = unknown>(
	method: string,
	input: unknown = {},
	options?: Omit<UseQueryOptions<T, Error>, "queryKey" | "queryFn">,
) {
	const client = useMobileClient();
	const libraryId = client.getCurrentLibraryId();

	return useQuery<T, Error>({
		queryKey: ["library", libraryId, method, input],
		queryFn: () => client.libraryQuery<T>(method, input),
		enabled: !!libraryId && (options?.enabled ?? true),
		...options,
	});
}

/**
 * Hook for executing core-level actions (mutations).
 */
export function useCoreAction<TInput = unknown, TOutput = unknown>(
	method: string,
	options?: UseMutationOptions<TOutput, Error, TInput>,
) {
	const client = useMobileClient();

	return useMutation<TOutput, Error, TInput>({
		mutationFn: (input: TInput) =>
			client.coreAction<TOutput>(method, input),
		...options,
	});
}

/**
 * Hook for executing library-level actions (mutations).
 */
export function useLibraryAction<TInput = unknown, TOutput = unknown>(
	method: string,
	options?: UseMutationOptions<TOutput, Error, TInput>,
) {
	const client = useMobileClient();

	return useMutation<TOutput, Error, TInput>({
		mutationFn: (input: TInput) =>
			client.libraryAction<TOutput>(method, input),
		...options,
	});
}
