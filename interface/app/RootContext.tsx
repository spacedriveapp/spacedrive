import { createContext, useContext } from 'react';

interface RootContext {
	rawPath: string;
}

/**
 * Provides data that should be accessible to all routes but is not platform-specific.
 */
export const RootContext = createContext<RootContext | null>(null);

export const useRootContext = () => {
	const ctx = useContext(RootContext);

	if (!ctx) throw new Error('RootContext.Provider not found!');

	return ctx;
};
