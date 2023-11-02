import { initRspc, ProcedureDef } from '@rspc/client';
import { createReactQueryHooks } from '@rspc/react-query';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { PropsWithChildren } from 'react';
import { match, P } from 'ts-pattern';

import { LibraryArgs, Procedures } from './core';
import { currentLibraryCache } from './hooks';

type NonLibraryProcedure<T extends keyof Procedures> =
	| Exclude<Procedures[T], { input: LibraryArgs<any> }>
	| Extract<Procedures[T], { input: never }>;

type LibraryProcedures<T extends keyof Procedures> = Exclude<
	Extract<Procedures[T], { input: LibraryArgs<any> }>,
	{ input: never }
>;

type StripLibraryArgsFromInput<
	T extends ProcedureDef,
	NeverOverNull extends boolean
> = T extends any
	? T['input'] extends LibraryArgs<infer E>
		? {
				key: T['key'];
				input: NeverOverNull extends true ? (E extends null ? never : E) : E;
				result: T['result'];
				error: T['error'];
		  }
		: never
	: never;

type NonLibraryProceduresDef = {
	queries: NonLibraryProcedure<'queries'>;
	mutations: NonLibraryProcedure<'mutations'>;
	subscriptions: NonLibraryProcedure<'subscriptions'>;
};

export type LibraryProceduresDef = {
	queries: StripLibraryArgsFromInput<LibraryProcedures<'queries'>, true>;
	mutations: StripLibraryArgsFromInput<LibraryProcedures<'mutations'>, false>;
	subscriptions: StripLibraryArgsFromInput<LibraryProcedures<'subscriptions'>, true>;
};

export const rspc = initRspc<Procedures>({
	links: globalThis.rspcLinks
});
export const rspc2 = initRspc<Procedures>({
	links: globalThis.rspcLinks
}); // TODO: Removing this?

export const nonLibraryClient = rspc.dangerouslyHookIntoInternals<NonLibraryProceduresDef>();

const nonLibraryHooks = createReactQueryHooks<NonLibraryProceduresDef>();

export const libraryClient = rspc2.dangerouslyHookIntoInternals<LibraryProceduresDef>({
	mapQueryKey: (keyAndInput) => {
		const libraryId = currentLibraryCache.id;
		if (libraryId === null)
			throw new Error('Attempted to do library operation with no library set!');
		return [keyAndInput[0], { library_id: libraryId, arg: keyAndInput[1] ?? null }];
	}
});

const libraryHooks = createReactQueryHooks<LibraryProceduresDef>();

// TODO: Allow both hooks to use a unified context -> Right now they override each others local state
export function RspcProvider({
	queryClient,
	children
}: PropsWithChildren<{ queryClient: QueryClient }>) {
	return (
		<QueryClientProvider client={queryClient}>
			<libraryHooks.Provider client={libraryClient} queryClient={queryClient}>
				<nonLibraryHooks.Provider client={nonLibraryClient} queryClient={queryClient}>
					{children as any}
				</nonLibraryHooks.Provider>
			</libraryHooks.Provider>
		</QueryClientProvider>
	);
}

export const useBridgeQuery = nonLibraryHooks.useQuery;
export const useBridgeMutation = nonLibraryHooks.useMutation;
export const useBridgeSubscription = nonLibraryHooks.useSubscription;
export const useLibraryQuery = libraryHooks.useQuery;
export const useLibraryMutation = libraryHooks.useMutation;
export const useLibrarySubscription = libraryHooks.useSubscription;

export function useInvalidateQuery() {
	const context = nonLibraryHooks.useContext();
	useBridgeSubscription(['invalidation.listen'], {
		onData: (ops) => {
			for (const op of ops) {
				match(op)
					.with({ type: 'single', data: P.select() }, (op) => {
						let key = [op.key];
						if (op.arg !== null) {
							key = key.concat(op.arg);
						}

						if (op.result !== null) {
							context.queryClient.setQueryData(key, op.result);
						} else {
							context.queryClient.invalidateQueries({
								queryKey: key
							});
						}
					})
					.with({ type: 'all' }, (op) => {
						context.queryClient.invalidateQueries();
					})
					.exhaustive();
			}
		}
	});
}

// TODO: Remove/fix this when rspc typesafe errors are working
export function extractInfoRSPCError(error: unknown) {
	// if (!(error instanceof AlphaRSPCError)) return null;
	// return error;

	throw new Error('TODO');
}
