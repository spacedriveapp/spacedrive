import { UseInfiniteQueryOptions } from '@tanstack/react-query';
import { ExplorerItem, LibraryConfigWrapped, SearchData } from '@sd/client';

import { Ordering } from '../store';
import { UseExplorerSettings } from '../useExplorer';

export type UseExplorerInfiniteQueryArgs<TArg, TOrder extends Ordering> = {
	library: LibraryConfigWrapped;
	arg: TArg;
	settings: UseExplorerSettings<TOrder>;
} & Pick<UseInfiniteQueryOptions<SearchData<ExplorerItem>>, 'enabled'>;
