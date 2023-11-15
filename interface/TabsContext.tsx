import { createContext, useContext } from 'react';

import { Router } from './';

export const TabsContext = createContext<{
	routerIndex: number;
	setRouterIndex: (i: number) => void;
	routers: Router[];
	createRouter(): void;
	removeRouter(index: number): void;
} | null>(null);

export function useTabsContext() {
	const ctx = useContext(TabsContext);

	return ctx;
}
