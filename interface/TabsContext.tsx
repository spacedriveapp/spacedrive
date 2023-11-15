import { createContext, Dispatch, SetStateAction, useContext } from 'react';

import { Router } from './';

export const TabsContext = createContext<{
	routerIndex: number;
	setRouterIndex: (i: number) => void;
	routers: Router[];
	setRouters: Dispatch<SetStateAction<Router[]>>;
	createRouter: () => Router;
} | null>(null);

export function useTabsContext() {
	const ctx = useContext(TabsContext);

	return ctx;
}
