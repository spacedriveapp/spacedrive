import { FilePathOrder, FilePathSearchArgs, useLibraryQuery } from '@sd/client';

import { UseExplorerSettings } from '../useExplorer';
import { useExplorerQuery } from './useExplorerQuery';
import { usePathsInfiniteQuery } from './usePathsInfiniteQuery';

export function usePathsExplorerQuery(props: {
	arg: FilePathSearchArgs;
	explorerSettings: UseExplorerSettings<FilePathOrder>;
}) {
	const query = usePathsInfiniteQuery(props);

	const count = useLibraryQuery(['search.pathsCount', { filters: props.arg.filters }], {
		enabled: query.isSuccess
	});

	return useExplorerQuery(query, count);
}
