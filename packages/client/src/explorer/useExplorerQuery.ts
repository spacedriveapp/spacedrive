import { InfiniteData, UseInfiniteQueryResult, UseQueryResult } from '@tanstack/react-query';
import { useCallback, useMemo } from 'react';

import { SearchData } from '../core';

export function useExplorerQuery<Q>(
	query: UseInfiniteQueryResult<InfiniteData<SearchData<Q>>>,
	count: UseQueryResult<number>
) {
	const items = useMemo(
		() => query.data?.pages.flatMap(data => data.items) ?? null,
		[query.data]
	);

	const loadMore = useCallback(() => {
		if (query.hasNextPage && !query.isFetchingNextPage) {
			query.fetchNextPage.call(undefined);
		}
	}, [query.hasNextPage, query.isFetchingNextPage, query.fetchNextPage]);

	return { query, items, loadMore, count: count.data };
}

export type UseExplorerQuery<Q> = ReturnType<typeof useExplorerQuery<Q>>;
