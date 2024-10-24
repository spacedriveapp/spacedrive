import { useInfiniteQuery } from '@tanstack/react-query';

import { FilePathOrder, FilePathSearchArgs } from '../core';
import { useLibraryContext } from '../hooks';
import { useRspcLibraryContext } from '../rspc';
import { UseExplorerInfiniteQueryArgs } from './useExplorerInfiniteQuery';

export function usePathsOffsetInfiniteQuery({
	arg,
	order
}: UseExplorerInfiniteQueryArgs<FilePathSearchArgs, FilePathOrder>) {
	const take = arg.take ?? 100;

	const { library } = useLibraryContext();
	const ctx = useRspcLibraryContext();

	if (order) {
		arg.orderAndPagination = { orderOnly: order };
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

			return { ...result, offset: pageParam, arg };
		},
		initialPageParam: 0,
		getNextPageParam: ({ items, offset, arg }) => {
			if (items.length >= arg.take) return (offset ?? 0) + 1;
		}
	});

	return query;
}
