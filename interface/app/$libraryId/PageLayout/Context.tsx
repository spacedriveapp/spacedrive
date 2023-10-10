import { createContext, RefObject, useContext } from 'react';

/**
 * Context to hold the ref value of the page layout
 */
export const PageLayoutContext = createContext<{ ref: RefObject<HTMLDivElement> } | null>(null);

export const usePageLayoutContext = () => {
	const ctx = useContext(PageLayoutContext);

	if (ctx === null) throw new Error('PageLayoutContext.Provider not found!');

	return ctx;
};
