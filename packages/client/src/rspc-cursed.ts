import { _inferProcedureHandlerInput, inferProcedureResult } from '@rspc/client';
import { useQuery, UseQueryOptions, UseQueryResult } from '@tanstack/react-query';
import { useRef } from 'react';

import { Procedures } from './core';
import { useRspcContext } from './rspc';

// A query where the data is streamed in.
// Also basically a subscription with support for React Suspense and proper loading states, invalidation, etc.
// Be aware this lacks proper type safety and is an absolutely cursed abomination of code.
//
// It requires you using `UnsafeStreamedQuery` on the backend and will not type error if you don't hence unsafe.
// It also requires special modification to the invalidation system to work correctly.
//
// Be aware `.streaming` will be emptied on a refetch so you should only use it when `.data` is not available.
export function useUnsafeStreamedQuery<
	K extends Procedures['subscriptions']['key'] & string,
	TData = inferProcedureResult<Procedures, 'subscriptions', K>
>(
	keyAndInput: [K, ..._inferProcedureHandlerInput<Procedures, 'subscriptions', K>],
	opts: UseQueryOptions<TData[]> & {
		onBatch(item: TData): void;
	}
): UseQueryResult<TData[], unknown> & { streaming: TData[] } {
	const data = useRef<TData[]>([]);
	const rspc = useRspcContext();

	// TODO: The normalised cache might cleanup nodes for this query before it's finished streaming. We need a global mutex on the cleanup routine.

	const query = useQuery({
		queryKey: keyAndInput,
		queryFn: ({ signal }) =>
			new Promise((resolve) => {
				data.current = [];
				const shutdown = rspc.client.addSubscription(keyAndInput as any, {
					onData: (item) => {
						if (item === null || item === undefined) return;

						if ('__stream_complete' in item) {
							resolve(data.current as any);
							return;
						}

						opts.onBatch(item as any);
						data.current.push(item as any);
					}
				});
				signal?.addEventListener('abort', () => shutdown());
			}),
		...opts
	});

	return {
		...query,
		streaming: data.current
	};
}
