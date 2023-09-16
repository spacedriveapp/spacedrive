import { createContext, RefObject, useContext } from 'react';

/**
 * Context to hold the ref value of the layout for styling manipulation
 */
export const LayoutContext = createContext<{ ref: RefObject<HTMLDivElement> } | null>(null);

export const useLayoutContext = () => {
	const ctx = useContext(LayoutContext);

	if (ctx === null) throw new Error('LayoutContext.Provider not found!');

	return ctx;
};
