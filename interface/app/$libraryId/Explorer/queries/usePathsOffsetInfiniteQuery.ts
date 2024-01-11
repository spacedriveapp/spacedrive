import { useInfiniteQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import {
	FilePathOrder,
	FilePathSearchArgs,
	useLibraryContext,
	useNodes,
	useNormalisedCache,
	useRspcLibraryContext
} from '@sd/client';

import { explorerStore } from '../store';
import { UseExplorerInfiniteQueryArgs } from './useExplorerInfiniteQuery';

export function usePathsOffsetInfiniteQuery({
	arg,
	explorerSettings,
	...args
}: UseExplorerInfiniteQueryArgs<FilePathSearchArgs, FilePathOrder>) {
	const take = arg.take ?? 100;

	const { library } = useLibraryContext();
	const ctx = useRspcLibraryContext();
	const settings = explorerSettings.useSettingsSnapshot();
	const cache = useNormalisedCache();

	if (settings.order) {
		arg.orderAndPagination = { orderOnly: settings.order };
		if (arg.orderAndPagination.orderOnly.field === 'sizeInBytes') delete arg.take;
	}

	const query = useInfiniteQuery({
		queryKey: [
			'search.paths',
			{
				library_id: library.uuid,
				arg: { ...arg, take }
			}
		] satisfies [any, any],
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

			const result = await ctx.client.query(['search.paths', arg]);
			cache.withNodes(result.nodes);

			return { ...result, offset: pageParam, arg };
		},
		getNextPageParam: ({ nodes, offset, arg }) => {
			if (nodes.length >= arg.take) return (offset ?? 0) + 1;
		},
		onSuccess: () => explorerStore.resetNewThumbnails(),
		...args
	});

	const nodes = useMemo(
		() => query.data?.pages.flatMap((page) => page.nodes) ?? [],
		[query.data?.pages]
	);

	useNodes(nodes);

	return query;
}
