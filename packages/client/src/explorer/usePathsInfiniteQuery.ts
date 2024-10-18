import { useInfiniteQuery } from '@tanstack/react-query';

import {
	ExplorerItem,
	FilePathCursorVariant,
	FilePathObjectCursor,
	FilePathOrder,
	FilePathSearchArgs
} from '../core';
import { useRspcLibraryContext } from '../rspc';
import { UseExplorerInfiniteQueryArgs } from './useExplorerInfiniteQuery';

export function usePathsInfiniteQuery({
	arg,
	order
}: UseExplorerInfiniteQueryArgs<FilePathSearchArgs, FilePathOrder>) {
	const ctx = useRspcLibraryContext();

	if (order) {
		arg.orderAndPagination = { orderOnly: order };
		if (arg.orderAndPagination.orderOnly.field === 'sizeInBytes') delete arg.take;
	}

	const query = useInfiniteQuery({
		queryKey: ['search.paths'],
		queryFn: async ({ pageParam }) => {
			let orderAndPagination: (typeof arg)['orderAndPagination'];
			if (!pageParam || pageParam.type !== 'Path') {
				if (order) orderAndPagination = { orderOnly: order };
			} else {
				let variant: FilePathCursorVariant | undefined;
				if (!order) variant = 'none';
				else if (pageParam) {
					switch (order.field) {
						case 'name': {
							const data = pageParam.item.name;
							if (data !== null)
								variant = {
									name: { order: order.value, data }
								};
							break;
						}
						case 'sizeInBytes': {
							variant = { sizeInBytes: order.value };
							break;
						}
						case 'dateCreated': {
							const data = pageParam.item.date_created;
							if (data !== null)
								variant = {
									dateCreated: { order: order.value, data }
								};
							break;
						}
						case 'dateModified': {
							const data = pageParam.item.date_modified;
							if (data !== null)
								variant = {
									dateModified: { order: order.value, data }
								};
							break;
						}
						case 'dateIndexed': {
							const data = pageParam.item.date_indexed;
							if (data !== null)
								variant = {
									dateIndexed: { order: order.value, data }
								};
							break;
						}
						case 'object': {
							const object = pageParam.item.object;
							if (!object) break;
							let objectCursor: FilePathObjectCursor | undefined;
							switch (order.value.field) {
								case 'dateAccessed': {
									const data = object.date_accessed;
									if (data !== null)
										objectCursor = {
											dateAccessed: { order: order.value.value, data }
										};
									break;
								}
								case 'kind': {
									const data = object.kind;
									if (data !== null)
										objectCursor = {
											kind: { order: order.value.value, data }
										};
									break;
								}
							}
							if (objectCursor) variant = { object: objectCursor };
							break;
						}
					}
				}
				if (pageParam.item.is_dir === null) throw new Error();
				if (variant)
					orderAndPagination = {
						cursor: {
							cursor: { variant, isDir: pageParam.item.is_dir },
							id: pageParam.item.id
						}
					};
			}
			arg.orderAndPagination = orderAndPagination;
			const result = await ctx.client.query(['search.paths', arg]);
			return result;
		},
		initialPageParam: undefined as ExplorerItem | undefined,
		getNextPageParam: (lastPage) => {
			if (arg.take === null || arg.take === undefined) return undefined;
			if (lastPage.items.length < arg.take) return undefined;
			else return lastPage.items[arg.take - 1];
		}
	});

	return query;
}
