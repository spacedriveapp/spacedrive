import { useInfiniteQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import {
	ExplorerItem,
	ObjectOrder,
	ObjectSearchArgs,
	useLibraryContext,
	useNodes,
	useNormalisedCache,
	useRspcLibraryContext
} from '@sd/client';

import { UseExplorerInfiniteQueryArgs } from './useExplorerInfiniteQuery';

export function useObjectsOffsetInfiniteQuery({
	arg,
	explorerSettings,
	...args
}: UseExplorerInfiniteQueryArgs<ObjectSearchArgs, ObjectOrder>) {
	const { library } = useLibraryContext();
	const ctx = useRspcLibraryContext();
	const settings = explorerSettings.useSettingsSnapshot();
	const cache = useNormalisedCache();

	if (settings.order) {
		arg.orderAndPagination = { orderOnly: settings.order };
	}

	const query = useInfiniteQuery({
		queryKey: ['search.objects', { library_id: library.uuid, arg }] as const,
		queryFn: async ({ pageParam, queryKey: [_, { arg }] }) => {
			const { order } = settings;

			let orderAndPagination: (typeof arg)['orderAndPagination'];

			if (!pageParam) {
				if (order) orderAndPagination = { orderOnly: order };
			} else {
				orderAndPagination = {
					offset: {
						order,
						offset: pageParam * arg.take
					}
				};
			}

			arg.orderAndPagination = orderAndPagination;

			const result = await ctx.client.query(['search.objects', arg]);
			cache.withNodes(result.nodes);

			return { ...result, offset: pageParam, arg };
		},
		getNextPageParam: ({ nodes, offset, arg }) => {
			if (nodes.length >= arg.take) return (offset ?? 0) + 1;
		},
		...args
	});

	const nodes = useMemo(
		() => query.data?.pages.flatMap((page) => page.nodes) ?? [],
		[query.data?.pages]
	);

	useNodes(nodes);

	return query;
}
