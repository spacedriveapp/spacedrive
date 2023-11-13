import { ContextType, createContext, PropsWithChildren, useContext } from 'react';

import { Ordering } from './store';
import { UseExplorer } from './useExplorer';

/**
 * Context that must wrap anything to do with the explorer.
 * This includes explorer views, the inspector, and top bar items.
 */
const ExplorerContext = createContext<UseExplorer<Ordering> | null>(null);

type ExplorerContext = NonNullable<ContextType<typeof ExplorerContext>>;

export const useExplorerContext = <T extends boolean = true>(
	{ suspense }: { suspense?: T } = { suspense: true as T }
) => {
	const ctx = useContext(ExplorerContext);

	if (suspense && ctx === null) throw new Error('ExplorerContext.Provider not found!');

	return ctx as T extends true ? ExplorerContext : ExplorerContext | undefined;
};

export const ExplorerContextProvider = <TExplorer extends UseExplorer<any>>({
	explorer,
	children
}: PropsWithChildren<{
	explorer: TExplorer;
}>) => <ExplorerContext.Provider value={explorer as any}>{children}</ExplorerContext.Provider>;
