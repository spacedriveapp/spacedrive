import { RSPCError, createReactQueryHooks } from '@rspc/client';
import { LibraryArgs, Operations } from '@sd/core';
import {
	QueryClient,
	UseMutationOptions,
	UseMutationResult,
	UseQueryOptions,
	UseQueryResult,
	useMutation as _useMutation
} from '@tanstack/react-query';

import { useLibraryStore } from './stores';

export const queryClient = new QueryClient();
export const rspc = createReactQueryHooks<Operations>();

type NonLibraryQueries = Exclude<Operations['queries'], { key: [any, LibraryArgs<any>] }> &
	Extract<Operations['queries'], { key: [any] }>;
type NonLibraryQuery<K extends string> = Extract<NonLibraryQueries, { key: [K] | [K, any] }>;
type NonLibraryQueryKey = NonLibraryQueries['key'][0];
type NonLibraryQueryResult<K extends NonLibraryQueryKey> = NonLibraryQuery<K>['result'];

export function useBridgeQuery<K extends NonLibraryQueries['key']>(
	key: K,
	options?: UseQueryOptions<NonLibraryQueryResult<K[0]>, RSPCError>
): UseQueryResult<NonLibraryQueryResult<K[0]>, RSPCError> {
	// @ts-ignore
	return rspc.useQuery(key, options);
}

type LibraryQueries = Extract<Operations['queries'], { key: [string, LibraryArgs<any>] }>;
type LibraryQuery<K extends string> = Extract<LibraryQueries, { key: [K, any] }>;
type LibraryQueryResult<K extends string> = LibraryQuery<K>['result'];

export function useLibraryQuery<K extends LibraryQueries['key']>(
	key: K,
	options?: UseQueryOptions<LibraryQueryResult<K[0]>, RSPCError>
): UseQueryResult<LibraryQueryResult<K[0]>, RSPCError> {
	const library_id = useLibraryStore((state) => state.currentLibraryUuid);
	if (!library_id) throw new Error(`Attempted to do library query with no library set!`);

	// @ts-ignore
	return rspc.useQuery([key[0], { library_id: library_id || '', arg: key[1] || null }], options);
}

type LibraryMutations = Extract<Operations['mutations'], { key: [string, LibraryArgs<any>] }>;
type LibraryMutation<K extends LibraryMutationKey> = Extract<LibraryMutations, { key: [K, any] }>;
type LibraryMutationKey = LibraryMutations['key'][0];
type LibraryMutationResult<K extends LibraryMutations['key'][0]> = LibraryMutation<K>['result'];

export function useLibraryMutation<K extends LibraryMutations['key']>(
	key: K,
	options?: UseMutationOptions<LibraryMutationResult<K[0]>, RSPCError, K[1]['arg']>
): UseMutationResult<LibraryMutationResult<K[0]>, RSPCError, K[1]> {
	const ctx = rspc.useContext();
	const library_id = useLibraryStore((state) => state.currentLibraryUuid);
	if (!library_id) throw new Error(`Attempted to do library query with no library set!`);

	// @ts-ignore
	return _useMutation(
		async (data) =>
			ctx.client.mutation([key[0], { library_id: library_id || '', arg: data || null }]),
		{
			...options,
			// @ts-ignore
			context: rspc.ReactQueryContext
		}
	);
}

type NonLibraryMutations = Exclude<Operations['mutations'], { key: [any, LibraryArgs<any>] }>;
type NonLibraryMutation<K extends NonLibraryMutationKey> = Extract<
	NonLibraryMutations,
	{ key: [K] | [K, any] }
>;
type NonLibraryMutationKey = NonLibraryMutations['key'][0];
type NonLibraryMutationResult<K extends NonLibraryMutationKey> = NonLibraryMutation<K>['result'];

export function useBridgeMutation<K extends NonLibraryMutations['key']>(
	key: K,
	options?: UseMutationOptions<NonLibraryMutationResult<K[0]>, RSPCError, K[1]>
): UseMutationResult<NonLibraryMutationResult<K[0]>, RSPCError, K[1]> {
	// @ts-ignore
	return rspc.useMutation(key, options);
}

export function useInvalidateQuery() {
	const context = rspc.useContext();
	rspc.useSubscription(['invalidateQuery'], {
		onNext: (invalidateOperation) => {
			let key = [invalidateOperation.key];
			if (invalidateOperation.arg !== null) {
				key.concat(invalidateOperation.arg);
			}
			context.queryClient.invalidateQueries(key);
		}
	});
}
