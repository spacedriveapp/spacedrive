import { UseInfiniteQueryOptions } from '@tanstack/react-query';
import { ExplorerItem, SearchData } from '@sd/client';

import { Ordering } from '../store';
import { UseExplorerSettings } from '../useExplorer';

export type UseExplorerInfiniteQueryArgs<TArg, TOrder extends Ordering> = {
	arg: TArg;
	explorerSettings: UseExplorerSettings<TOrder>;
} & Pick<UseInfiniteQueryOptions<SearchData<ExplorerItem>>, 'enabled' | 'suspense'>;
