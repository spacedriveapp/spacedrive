import { FilePathOrder, FilePathSearchArgs } from '../core';
import { useLibraryQuery } from '../rspc';
import { useExplorerQuery } from './useExplorerQuery';
import { usePathsOffsetInfiniteQuery } from './usePathsOffsetInfiniteQuery';

export function usePathsExplorerQuery(props: {
	arg: FilePathSearchArgs;
	order: FilePathOrder | null;
	enabled?: boolean;
	suspense?: boolean;
}) {
	const query = usePathsOffsetInfiniteQuery(props);

	const count = useLibraryQuery(['search.pathsCount', { filters: props.arg.filters }], {
		enabled: query.isSuccess
	});

	return useExplorerQuery(query, count);
}
