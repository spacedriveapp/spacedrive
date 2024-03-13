import { useInfiniteQuery } from '@tanstack/react-query';
import { useMemo } from 'react';

import { useNodes } from '../cache';
import { ExplorerItem, ObjectCursor, ObjectOrder, ObjectSearchArgs } from '../core';
import { useLibraryContext } from '../hooks';
import { useRspcLibraryContext } from '../rspc';
import { UseExplorerInfiniteQueryArgs } from './useExplorerInfiniteQuery';

export function useObjectsInfiniteQuery({
	arg,
	order,
	...args
}: UseExplorerInfiniteQueryArgs<ObjectSearchArgs, ObjectOrder>) {
	const { library } = useLibraryContext();
	const ctx = useRspcLibraryContext();

	if (order) {
		arg.orderAndPagination = { orderOnly: order };
	}

	const query = useInfiniteQuery({
		queryKey: ['search.objects', { library_id: library.uuid, arg }] as const,
		queryFn: ({ pageParam, queryKey: [_, { arg }] }) => {
			const cItem: Extract<ExplorerItem, { type: 'Object' }> = pageParam;

			let orderAndPagination: (typeof arg)['orderAndPagination'];

			if (!cItem) {
				if (order) orderAndPagination = { orderOnly: order };
			} else {
				let cursor: ObjectCursor | undefined;

				if (!order) cursor = 'none';
				else if (cItem) {
					switch (order.field) {
						case 'kind': {
							const data = cItem.item.kind;
							if (data !== null) cursor = { kind: { order: order.value, data } };
							break;
						}
						case 'dateAccessed': {
							const data = cItem.item.date_accessed;
							if (data !== null)
								cursor = { dateAccessed: { order: order.value, data } };
							break;
						}
					}
				}

				if (cursor) orderAndPagination = { cursor: { cursor, id: cItem.item.id } };
			}

			arg.orderAndPagination = orderAndPagination;

			return ctx.client.query(['search.objects', arg]);
		},
		getNextPageParam: (lastPage) => {
			if (lastPage.items.length < arg.take) return undefined;
			else return lastPage.nodes[arg.take - 1];
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
