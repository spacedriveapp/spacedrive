import { createContext, useContext } from 'react';
import { UseExplorer } from './useExplorer';

/**
 * Context that must wrap anything to do with the explorer.
 * This includes explorer views, the inspector, and top bar items.
 */
export const ExplorerContext = createContext<UseExplorer | null>(null);

export const useExplorerContext = () => {
	const ctx = useContext(ExplorerContext);

	if (ctx === null) throw new Error('ExplorerContext.Provider not found!');

	return ctx;
};
