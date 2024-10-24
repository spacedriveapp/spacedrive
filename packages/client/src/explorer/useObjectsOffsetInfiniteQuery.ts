import { useInfiniteQuery } from '@tanstack/react-query';
import { useMemo } from 'react';

import { ObjectOrder, ObjectSearchArgs } from '../core';
import { useLibraryContext } from '../hooks';
import { useRspcLibraryContext } from '../rspc';
import { UseExplorerInfiniteQueryArgs } from './useExplorerInfiniteQuery';

export function useObjectsOffsetInfiniteQuery({
	arg,
	order
}: UseExplorerInfiniteQueryArgs<ObjectSearchArgs, ObjectOrder>) {
	const { library } = useLibraryContext();
	const ctx = useRspcLibraryContext();

	if (order) {
		arg.orderAndPagination = { orderOnly: order };
	}

	const query = useInfiniteQuery({
		queryKey: ['search.objects', { library_id: library.uuid, arg }] as const,
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

			const result = await ctx.client.query(['search.objects', arg]);

			return { ...result, offset: pageParam, arg };
		},
		initialPageParam: 0,
		getNextPageParam: ({ items, offset, arg }) => {
			if (items.length >= arg.take) return (offset ?? 0) + 1;
		}
	});

	return query;
}
