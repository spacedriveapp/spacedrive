import { FilePathOrder, FilePathSearchArgs, useLibraryQuery } from '@sd/client';

import { UseExplorerSettings } from '../useExplorer';
import { useExplorerQuery } from './useExplorerQuery';
import { usePathsOffsetInfiniteQuery } from './usePathsOffsetInfiniteQuery';

export function usePathsExplorerQuery(props: {
	arg: FilePathSearchArgs;
	explorerSettings: UseExplorerSettings<FilePathOrder>;
}) {
	const query = usePathsOffsetInfiniteQuery(props);

	const count = useLibraryQuery(['search.pathsCount', { filters: props.arg.filters }], {
		enabled: query.isSuccess
	});

	return useExplorerQuery(query, count);
}
