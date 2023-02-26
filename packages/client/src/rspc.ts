import { ProcedureDef } from '@rspc/client';
import { internal_createReactHooksFactory } from '@rspc/react';
import { QueryClient } from '@tanstack/react-query';
import { LibraryArgs, Procedures } from './core';
import { currentLibraryCache } from './hooks';
import { normiCustomHooks } from './normi';

type NonLibraryProcedure<T extends keyof Procedures> =
	| Exclude<Procedures[T], { input: LibraryArgs<any> }>
	| Extract<Procedures[T], { input: never }>;

type LibraryProcedures<T extends keyof Procedures> = Exclude<
	Extract<Procedures[T], { input: LibraryArgs<any> }>,
	{ input: never }
>;

type StripLibraryArgsFromInput<T extends ProcedureDef> = T extends any
	? T['input'] extends LibraryArgs<infer E>
		? {
				key: T['key'];
				input: E;
				result: T['result'];
		  }
		: never
	: never;

let getLibraryId: () => string | null;

export const setLibraryIdGetter = (g: typeof getLibraryId) => (getLibraryId = g);

export const hooks = internal_createReactHooksFactory();

const nonLibraryHooks = hooks.createHooks<
	Procedures,
	// Normalized<NonLibraryProcedure<'queries'>>,
	// Normalized<NonLibraryProcedure<'mutations'>>
	NonLibraryProcedure<'queries'>,
	NonLibraryProcedure<'mutations'>
>({
	internal: {
		customHooks: normiCustomHooks({ contextSharing: true })
	}
});

const libraryHooks = hooks.createHooks<
	Procedures,
	// Normalized<StripLibraryArgsFromInput<LibraryProcedures<'queries'>>>,
	// Normalized<StripLibraryArgsFromInput<LibraryProcedures<'mutations'>>>,
	StripLibraryArgsFromInput<LibraryProcedures<'queries'>>,
	StripLibraryArgsFromInput<LibraryProcedures<'mutations'>>,
	StripLibraryArgsFromInput<LibraryProcedures<'subscriptions'>>
>({
	internal: {
		customHooks: normiCustomHooks({ contextSharing: true }, () => {
			return {
				mapQueryKey: (keyAndInput) => {
					const libraryId = currentLibraryCache.id;
					if (libraryId === null)
						throw new Error('Attempted to do library operation with no library set!');
					return [keyAndInput[0], { library_id: libraryId, arg: keyAndInput[1] || null }];
				},
				doMutation: (keyAndInput, next) => {
					const libraryId = currentLibraryCache.id;
					if (libraryId === null)
						throw new Error('Attempted to do library operation with no library set!');
					return next([keyAndInput[0], { library_id: libraryId, arg: keyAndInput[1] || null }]);
				}
			};
		})
	}
});

export const queryClient = new QueryClient();
export const rspc = hooks.createHooks<Procedures>();

export { QueryClientProvider } from '@tanstack/react-query';

export const useBridgeQuery = nonLibraryHooks.useQuery;
export const useBridgeMutation = nonLibraryHooks.useMutation;
export const useBridgeSubscription = nonLibraryHooks.useSubscription;
export const useLibraryQuery = libraryHooks.useQuery;
export const useLibraryMutation = libraryHooks.useMutation;

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
