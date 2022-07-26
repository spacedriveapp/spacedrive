import { createReactQueryHooks } from '@rspc/client';
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

type LibraryQueries = Extract<Operations['queries'], { key: [string, LibraryArgs<any>] }>;
type LibraryQuery<K extends string> = Extract<LibraryQueries, { key: [K, any] }>;
type LibraryQueryKey = LibraryQueries['key'][0];
type LibraryQueryArgs<K extends string> = LibraryQuery<K>['key'][1] extends LibraryArgs<infer A>
	? A
	: never;
type LibraryQueryResult<K extends string> = LibraryQuery<K>['result'];

type NonLibraryQueries = Exclude<Operations['queries'], { key: [any, LibraryArgs<any>] }>;
type NonLibraryQuery<K extends string> = Extract<
	NonLibraryQueries,
	{ key: [K] } | { key: [K, any] }
>;
type NonLibraryQueryKey = NonLibraryQueries['key'][0];
type NonLibraryQueryArgs<K extends string> = NonLibraryQuery<K>['key'][1];
type NonLibraryQueryResult<K extends string> = NonLibraryQuery<K>['result'];

type LibraryMutations = Extract<Operations['mutations'], { key: [string, LibraryArgs<any>] }>;
type LibraryMutation<K extends string> = Extract<LibraryMutations, { key: [K, any] }>;
type LibraryMutationKey = LibraryMutations['key'][0];
type LibraryMutationArgs<K extends string> = LibraryMutation<K>['key'][1] extends LibraryArgs<
	infer A
>
	? A
	: never;
type LibraryMutationResult<K extends string> = LibraryMutation<K>['result'];

type NonLibraryMutations = Exclude<Operations['mutations'], { key: [any, LibraryArgs<any>] }>;
type NonLibraryMutation<K extends string> = Extract<NonLibraryMutations, { key: [K, any] }>;
type NonLibraryMutationKey = NonLibraryMutations['key'][0];
type NonLibraryMutationArgs<K extends string> = NonLibraryMutation<K>['key'][1];
type NonLibraryMutationResult<K extends string> = NonLibraryMutation<K>['result'];

export function useBridgeQuery<K extends NonLibraryQueryKey>(
	key: NonLibraryQueryArgs<K> extends null | undefined ? [K] : [K, NonLibraryQueryArgs<K>],
	options?: UseQueryOptions<NonLibraryQueryResult<K>>
): UseQueryResult<NonLibraryQueryResult<K>> {
	return rspc.useQuery(key, options);
}

export function useLibraryQuery<K extends LibraryQueryKey>(
	key: LibraryQueryArgs<K> extends null | undefined ? [K] : [K, LibraryQueryArgs<K>],
	options?: UseQueryOptions<LibraryQueryResult<K>>
): UseQueryResult<LibraryQueryResult<K>> {
	const library_id = useLibraryStore((state) => state.currentLibraryUuid);
	if (!library_id) throw new Error(`Attempted to do library query with no library set!`);
	return rspc.useQuery([key[0], { library_id: library_id || '', arg: key[1] || null }], options);
}

export function useLibraryCommand<K extends LibraryMutationKey>(
	key: K,
	options?: UseMutationOptions<LibraryMutationResult<K>>
): UseMutationResult<LibraryMutationResult<K>, never, LibraryMutationArgs<K>> {
	const ctx = rspc.useContext();
	const library_id = useLibraryStore((state) => state.currentLibraryUuid);
	if (!library_id) throw new Error(`Attempted to do library query with no library set!`);

	// @ts-ignore // TODO: Fix this
	return _useMutation(async (data) => ctx.client.mutation([key, data]), {
		...options,
		context: rspc.ReactQueryContext
	});
}

export function useBridgeCommand<K extends NonLibraryMutationKey>(
	key: K,
	options?: UseMutationOptions<NonLibraryMutationResult<K>>
): UseMutationResult<NonLibraryMutationResult<K>, never, NonLibraryMutationArgs<K>> {
	return rspc.useMutation(key, options);
}

export function useInvalidateQuery() {
	rspc.useSubscription('invalidateQuery', {
		onValue: (v) => {
			console.log('BRUH', v);
		}
	});
}

// TODO: Work out a solution for removing this
// @ts-ignore
export function libraryCommand<
	// @ts-ignore
	K extends LibraryCommandKeyType,
	// @ts-ignore
	LC extends LCType<K>,
	// @ts-ignore
	CR extends CRType<K>
	// @ts-ignore
>(key: K, vars: ExtractParams<LC>): Promise<ExtractData<CR>> {
	const library_id = useLibraryStore((state) => state.currentLibraryUuid);
	if (!library_id) throw new Error(`Attempted to do library command '${key}' with no library set!`);
	// @ts-ignore
	return commandBridge('LibraryCommand', { library_id, command: { key, params: vars } as any });
}
