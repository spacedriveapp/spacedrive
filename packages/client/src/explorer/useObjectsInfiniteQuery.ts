import { useInfiniteQuery } from '@tanstack/react-query';
import { useMemo } from 'react';

import { ExplorerItem, ObjectCursor, ObjectOrder, ObjectSearchArgs } from '../core';
import { useLibraryContext } from '../hooks';
import { useRspcLibraryContext } from '../rspc';
import { UseExplorerInfiniteQueryArgs } from './useExplorerInfiniteQuery';

export function useObjectsInfiniteQuery({
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
		queryFn: ({ pageParam, queryKey: [_, { arg }] }) => {
			let orderAndPagination: (typeof arg)['orderAndPagination'];

			if (!pageParam || pageParam.type !== 'Object') {
				if (order) orderAndPagination = { orderOnly: order };
			} else {
				let cursor: ObjectCursor | undefined;

				if (!order) cursor = 'none';
				else if (pageParam) {
					switch (order.field) {
						case 'kind': {
							const data = pageParam.item.kind;
							if (data !== null) cursor = { kind: { order: order.value, data } };
							break;
						}
						case 'dateAccessed': {
							const data = pageParam.item.date_accessed;
							if (data !== null)
								cursor = { dateAccessed: { order: order.value, data } };
							break;
						}
					}
				}

				if (cursor) orderAndPagination = { cursor: { cursor, id: pageParam.item.id } };
			}

			arg.orderAndPagination = orderAndPagination;

			return ctx.client.query(['search.objects', arg]);
		},
		initialPageParam: undefined as ExplorerItem | undefined,
		getNextPageParam: (lastPage) => {
			if (lastPage.items.length < arg.take) return undefined;
			else return lastPage.items[arg.take - 1];
		}
	});

	return query;
}
