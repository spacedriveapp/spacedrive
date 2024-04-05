import { UseInfiniteQueryResult, UseQueryResult } from '@tanstack/react-query';
import { useCallback, useMemo } from 'react';

import { useCache } from '../cache';
import { SearchData } from '../core';

export function useExplorerQuery<Q>(
	query: UseInfiniteQueryResult<SearchData<Q>>,
	count: UseQueryResult<number>
) {
	const items = useMemo(() => query.data?.pages.flatMap((d) => d.items) ?? null, [query.data]);

	const loadMore = useCallback(() => {
		if (query.hasNextPage && !query.isFetchingNextPage) {
			query.fetchNextPage.call(undefined);
		}
	}, [query.hasNextPage, query.isFetchingNextPage, query.fetchNextPage]);

	return { query, items: useCache(items), loadMore, count: count.data };
}

export type UseExplorerQuery<Q> = ReturnType<typeof useExplorerQuery<Q>>;
