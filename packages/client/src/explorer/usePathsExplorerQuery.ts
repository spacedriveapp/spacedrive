import { FilePathOrder, FilePathSearchArgs } from '../core';
import { useLibraryQuery } from '../rspc';
import { useExplorerQuery } from './useExplorerQuery';
import { usePathsOffsetInfiniteQuery } from './usePathsOffsetInfiniteQuery';

export function usePathsExplorerQuery(props: {
	arg: FilePathSearchArgs;
	order: FilePathOrder | null;
	/** This callback will fire any time the query successfully fetches new data. (NOTE: This will be removed on the next major version (react-query)) */
	onSuccess?: () => void;
}) {
	const query = usePathsOffsetInfiniteQuery(props);

	const count = useLibraryQuery(['search.pathsCount', { filters: props.arg.filters }], {
		enabled: query.isSuccess
	});

	return useExplorerQuery(query, count);
}
