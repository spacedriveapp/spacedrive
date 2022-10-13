import { ProcedureDef } from '@rspc/client';
import { createReactQueryHooks } from '@rspc/react';
import { QueryClient } from '@tanstack/react-query';

import { LibraryArgs, Procedures } from './core';
import { getLibraryIdRaw } from './index';

export const queryClient = new QueryClient();
export const rspc = createReactQueryHooks<Procedures>();

type NonLibraryProcedure<T extends keyof Procedures> =
	| Exclude<Procedures[T], { input: LibraryArgs<any> }>
	| Extract<Procedures[T], { input: never }>;

type LibraryProcedures<T extends keyof Procedures> = Exclude<
	Extract<Procedures[T], { input: LibraryArgs<any> }>,
	{ input: never }
>;

type MoreConstrainedQueries<T extends ProcedureDef> = T extends any
	? T['input'] extends LibraryArgs<infer E>
		? {
				key: T['key'];
				input: E;
				result: T['result'];
		  }
		: never
	: never;

export const useBridgeQuery = rspc.customQuery<NonLibraryProcedure<'queries'>>(
	(keyAndInput) => keyAndInput as any
);

export const useBridgeMutation = rspc.customMutation<NonLibraryProcedure<'mutations'>>(
	(keyAndInput) => keyAndInput
);

export const useLibraryQuery = rspc.customQuery<
	MoreConstrainedQueries<LibraryProcedures<'queries'>>
>((keyAndInput) => {
	const library_id = getLibraryIdRaw();
	if (library_id === null) throw new Error('Attempted to do library query with no library set!');
	return [keyAndInput[0], { library_id, arg: keyAndInput[1] || null }];
});

export const useLibraryMutation = rspc.customMutation<
	MoreConstrainedQueries<LibraryProcedures<'mutations'>>
>((keyAndInput) => {
	const library_id = getLibraryIdRaw();
	if (library_id === null) throw new Error('Attempted to do library query with no library set!');
	return [keyAndInput[0], { library_id, arg: keyAndInput[1] || null }];
});

export function useInvalidateQuery() {
	const context = rspc.useContext();
	rspc.useSubscription(['invalidateQuery'], {
		onData: (invalidateOperation) => {
			const key = [invalidateOperation.key];
			if (invalidateOperation.arg !== null) {
				key.concat(invalidateOperation.arg);
			}
			context.queryClient.invalidateQueries(key);
		}
	});
}
