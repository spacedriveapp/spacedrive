// `usePageContext` allows us to access `pageContext` in any React component.
// More infos: https://vite-plugin-ssr.com/pageContext-anywhere
import { ReactNode, createContext, useContext } from 'react';
import { PageContextBuiltIn } from 'vite-plugin-ssr';

import type { PageContext } from './types';

export { PageContextProvider };
export { usePageContext };

const Context = createContext<PageContextBuiltIn>(undefined as any);

function PageContextProvider({
	pageContext,
	children
}: {
	pageContext: PageContextBuiltIn;
	children: ReactNode;
}) {
	return <Context.Provider value={pageContext}>{children}</Context.Provider>;
}

function usePageContext() {
	const pageContext = useContext(Context);
	return pageContext;
}
