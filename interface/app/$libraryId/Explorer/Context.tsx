import { PropsWithChildren, createContext, useContext } from 'react';
import { Ordering } from './store';
import { UseExplorer } from './useExplorer';

/**
 * Context that must wrap anything to do with the explorer.
 * This includes explorer views, the inspector, and top bar items.
 */
const ExplorerContext = createContext<UseExplorer<Ordering> | null>(null);

export const useExplorerContext = () => {
	const ctx = useContext(ExplorerContext);

	if (ctx === null) throw new Error('ExplorerContext.Provider not found!');

	return ctx;
};

export const ExplorerContextProvider = <TOrdering extends Ordering>({
	explorer,
	children
}: PropsWithChildren<{
	explorer: UseExplorer<TOrdering>;
}>) => <ExplorerContext.Provider value={explorer as any}>{children}</ExplorerContext.Provider>;
