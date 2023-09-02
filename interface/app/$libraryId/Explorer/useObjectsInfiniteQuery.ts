import { UseInfiniteQueryOptions, useInfiniteQuery } from '@tanstack/react-query';
import {
	ExplorerItem,
	LibraryConfigWrapped,
	ObjectCursor,
	ObjectOrder,
	ObjectSearchArgs,
	OrderAndPagination,
	SearchData,
	useRspcLibraryContext
} from '@sd/client';
import { getExplorerStore } from './store';
import { UseExplorerSettings } from './useExplorer';

export function useObjectsInfiniteQuery({
	library,
	arg,
	settings,
	...args
}: {
	library: LibraryConfigWrapped;
	arg: ObjectSearchArgs;
	settings: UseExplorerSettings<ObjectOrder>;
} & Pick<UseInfiniteQueryOptions<SearchData<ExplorerItem>>, 'enabled'>) {
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

			let orderAndPagination: OrderAndPagination<ObjectOrder, ObjectCursor> | undefined;

			if (!cItem) {
				if (order) orderAndPagination = { orderOnly: order };
			} else {
				let cursor: ObjectCursor | undefined;

				if (!order) cursor = { none: [] };
				else if (cItem) {
					const direction = order.value;

					switch (order.field) {
						case 'kind': {
							const data = cItem.item.kind;
							if (data !== null) cursor = { kind: { order: direction, data } };
							break;
						}
						case 'dateAccessed': {
							const data = cItem.item.date_accessed;
							if (data !== null)
								cursor = { dateAccessed: { order: direction, data } };
							break;
						}
					}
				}

				if (cursor) orderAndPagination = { cursor };
			}

			arg.orderAndPagination = orderAndPagination;

			return ctx.client.query(['search.objects', arg]);
		},
		getNextPageParam: (lastPage) => {
			if (lastPage.items.length < arg.take) return undefined;
			else return lastPage.items[arg.take];
		},
		...args
	});
}
