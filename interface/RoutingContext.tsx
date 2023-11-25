import { createContext, useContext } from 'react';

import { createRoutes } from './app';

export const RoutingContext = createContext<{
	visible: boolean;
	currentIndex: number;
	maxIndex: number;
	routes: ReturnType<typeof createRoutes>;
} | null>(null);

export function useRoutingContext() {
	const ctx = useContext(RoutingContext);

	if (!ctx) throw new Error('useRoutingContext must be used within a RoutingContext.Provider');

	return ctx;
}
