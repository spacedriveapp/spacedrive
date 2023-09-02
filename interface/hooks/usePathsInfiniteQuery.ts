import { UseInfiniteQueryOptions, useInfiniteQuery } from '@tanstack/react-query';
import {
	ExplorerItem,
	FilePathCursorOrdering,
	FilePathOrderAndPaginationArgs,
	FilePathSearchArgs,
	FilePathSearchOrdering,
	LibraryConfigWrapped,
	ObjectCursorOrdering,
	SearchData,
	useRspcLibraryContext
} from '@sd/client';
import { getExplorerStore } from '~/app/$libraryId/Explorer/store';
import { UseExplorerSettings } from '~/app/$libraryId/Explorer/useExplorer';

export function usePathsInfiniteQuery({
	library,
	arg,
	settings,
	...args
}: {
	library: LibraryConfigWrapped;
	arg: FilePathSearchArgs;
	settings: UseExplorerSettings<FilePathSearchOrdering>;
} & Pick<UseInfiniteQueryOptions<SearchData<ExplorerItem>>, 'enabled'>) {
	const ctx = useRspcLibraryContext();
	const explorerSettings = settings.useSettingsSnapshot();

	if (explorerSettings.order) {
		arg.orderAndPagination = { orderOnly: explorerSettings.order };
	}

	return useInfiniteQuery({
		queryKey: ['search.paths', { library_id: library.uuid, arg }] as const,
		queryFn: ({ pageParam, queryKey: [_, { arg }] }) => {
			const cItem: Extract<ExplorerItem, { type: 'Path' }> = pageParam;
			const { order } = explorerSettings;

			let orderAndPagination: FilePathOrderAndPaginationArgs | undefined;

			if (!cItem) {
				if (order) orderAndPagination = { orderOnly: order };
			} else {
				let cursor_ordering: FilePathCursorOrdering | undefined;

				if (!order) cursor_ordering = { none: [] };
				else if (cItem) {
					switch (order.field) {
						case 'name': {
							const data = cItem.item.name;
							if (data !== null)
								cursor_ordering = {
									name: {
										order: order.value,
										data
									}
								};
							break;
						}
						case 'dateCreated': {
							const data = cItem.item.date_created;
							if (data !== null)
								cursor_ordering = {
									dateCreated: {
										order: order.value,
										data
									}
								};
							break;
						}
						case 'dateModified': {
							const data = cItem.item.date_modified;
							if (data !== null)
								cursor_ordering = {
									dateModified: {
										order: order.value,
										data
									}
								};
							break;
						}
						case 'dateIndexed': {
							const data = cItem.item.date_indexed;
							if (data !== null)
								cursor_ordering = {
									dateIndexed: {
										order: order.value,
										data
									}
								};
							break;
						}
						case 'object': {
							const object = cItem.item.object;
							if (!object) break;

							let objectCursor: ObjectCursorOrdering | undefined;

							switch (order.value.field) {
								case 'dateAccessed': {
									const data = object.date_accessed;
									if (data !== null)
										objectCursor = {
											dateAccessed: {
												order: order.value.value,
												data
											}
										};
									break;
								}
								case 'kind': {
									const data = object.kind;
									if (data !== null)
										objectCursor = {
											kind: {
												order: order.value.value,
												data
											}
										};
									break;
								}
							}

							if (objectCursor)
								cursor_ordering = {
									object: objectCursor
								};

							break;
						}
					}
				}

				if (cItem.item.is_dir === null) throw new Error();

				if (cursor_ordering)
					orderAndPagination = {
						cursor: { cursor_ordering, is_dir: cItem.item.is_dir }
					};
			}

			arg.orderAndPagination = orderAndPagination;

			return ctx.client.query(['search.paths', arg]);
		},
		getNextPageParam: (lastPage) => {
			if (lastPage.items.length < arg.take) return undefined;
			else return lastPage.items[arg.take];
		},
		onSuccess: () => getExplorerStore().resetNewThumbnails(),
		...args
	});
}
