import { ObjectOrder, ObjectSearchArgs } from '../core';
import { useLibraryQuery } from '../rspc';
import { useExplorerQuery } from './useExplorerQuery';
import { useObjectsOffsetInfiniteQuery } from './useObjectsOffsetInfiniteQuery';

export function useObjectsExplorerQuery(props: {
	arg: ObjectSearchArgs;
	order: ObjectOrder | null;
}) {
	const query = useObjectsOffsetInfiniteQuery(props);

	const count = useLibraryQuery(['search.objectsCount', { filters: props.arg.filters }], {
		enabled: query.isSuccess
	});

	return useExplorerQuery(query, count);
}
