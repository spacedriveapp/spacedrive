import { useInfiniteQuery } from '@tanstack/react-query';
import {
	ExplorerItem,
	ObjectCursor,
	ObjectOrder,
	ObjectSearchArgs,
	useRspcLibraryContext
} from '@sd/client';

import { UseExplorerInfiniteQueryArgs } from './useExplorerInfiniteQuery';

export function useObjectsInfiniteQuery({
	library,
	arg,
	settings,
	...args
}: UseExplorerInfiniteQueryArgs<ObjectSearchArgs, ObjectOrder>) {
	const ctx = useRspcLibraryContext();
	const explorerSettings = settings.useSettingsSnapshot();

	if (explorerSettings.order) {
		arg.orderAndPagination = { orderOnly: explorerSettings.order };
	}

	return useInfiniteQuery({
		queryKey: ['search.objects', { library_id: library.uuid, arg }] as const,
		queryFn: ({ pageParam, queryKey: [_, { arg }] }) => {
			const cItem: Extract<ExplorerItem, { type: 'Object' }> = pageParam;
			const { order } = explorerSettings;

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
			else return lastPage.items[arg.take - 1];
		},
		...args
	});
}
