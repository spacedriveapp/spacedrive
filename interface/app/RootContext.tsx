import { createContext, useContext } from 'react';

interface RootContext {
	rawPath: string;
}

export const RootContext = createContext<RootContext | null>(null);

export const useRootContext = () => {
	const ctx = useContext(RootContext);

	if (!ctx) throw new Error('RootContext.Provider not found!');

	return ctx;
};
