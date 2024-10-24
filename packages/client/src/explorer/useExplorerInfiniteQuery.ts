import { UseInfiniteQueryOptions } from '@tanstack/react-query';

import { ExplorerItem, SearchData } from '../core';
import { Ordering } from './index';

export type UseExplorerInfiniteQueryArgs<TArg, TOrder extends Ordering> = {
	arg: TArg;
	order: TOrder | null;
};
