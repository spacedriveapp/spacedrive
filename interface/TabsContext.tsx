import { createContext, useContext } from 'react';

import { Router } from './';

export const TabsContext = createContext<{
	router: Router;
	setRouterIndex: (i: number) => void;
	routers: Router[];
	setRouters: (routers: Router[]) => void;
	createRouter: () => Router;
} | null>(null);

export function useTabsContext() {
	const ctx = useContext(TabsContext);

	return ctx;
}
