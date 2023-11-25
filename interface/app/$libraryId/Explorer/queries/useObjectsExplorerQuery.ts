import { ObjectOrder, ObjectSearchArgs, useLibraryQuery } from '@sd/client';

import { UseExplorerSettings } from '../useExplorer';
import { useExplorerQuery } from './useExplorerQuery';
import { useObjectsInfiniteQuery } from './useObjectsInfiniteQuery';

export function useObjectsExplorerQuery(props: {
	arg: ObjectSearchArgs;
	explorerSettings: UseExplorerSettings<ObjectOrder>;
}) {
	const query = useObjectsInfiniteQuery(props);

	const count = useLibraryQuery(['search.objectsCount', { filters: props.arg.filters }], {
		enabled: query.isSuccess
	});

	return useExplorerQuery(query, count);
}
