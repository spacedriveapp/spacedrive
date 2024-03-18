import { type Router } from '@remix-run/router';
import { createContext, useContext } from 'react';

import { createRoutes } from './app';

export const RoutingContext = createContext<{
	visible: boolean;
	currentIndex: number;
	tabId: string;
	maxIndex: number;
	routes: ReturnType<typeof createRoutes>;
} | null>(null);

// We split this into a different context because we don't want to trigger the hook unnecessarily
export const RouterContext = createContext<Router | null>(null);

export function useRoutingContext() {
	const ctx = useContext(RoutingContext);

	if (!ctx) throw new Error('useRoutingContext must be used within a RoutingContext.Provider');

	return ctx;
}

export function useRouter() {
	const ctx = useContext(RouterContext);
	if (!ctx) throw new Error('useRouter must be used within a RouterContext.Provider');

	return ctx;
}
