import { createContext, useContext } from 'react';

export const RoutingContext = createContext<{
	currentIndex: number;
	maxIndex: number;
} | null>(null);

export function useRoutingContext() {
	const ctx = useContext(RoutingContext);

	if (!ctx) throw new Error('useRoutingContext must be used within a RoutingContext.Provider');

	return ctx;
}
