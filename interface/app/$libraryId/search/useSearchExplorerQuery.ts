import {
	FilePathOrder,
	FilePathSearchArgs,
	ObjectOrder,
	ObjectSearchArgs,
	SearchFilterArgs,
	useObjectsExplorerQuery,
	usePathsExplorerQuery
} from '@sd/client';

import { UseExplorerSettings } from '../Explorer/useExplorer';
import { UseSearch, UseSearchSource } from './useSearch';

export function useSearchExplorerQuery<TSource extends UseSearchSource>(props: {
	search: UseSearch<TSource>;
	explorerSettings: UseExplorerSettings<any, any>;
	filters: SearchFilterArgs[];
	take: number;
	paths?: { arg?: Omit<FilePathSearchArgs, 'filters' | 'take'>; order?: FilePathOrder | null };
	objects?: { arg?: Omit<ObjectSearchArgs, 'filters' | 'take'>; order?: ObjectOrder | null };
	onSuccess?: () => void;
}) {
	const filters = [...props.filters, ...props.explorerSettings.useLayoutSearchFilters()];

	if (props.search.target === 'paths') {
		return usePathsExplorerQuery({
			arg: { ...props.paths?.arg, filters, take: props.take },
			order: props.paths?.order ?? null
		});
	} else {
		return useObjectsExplorerQuery({
			arg: { ...props.objects?.arg, filters, take: props.take },
			order: props.objects?.order ?? null
		});
	}
}
