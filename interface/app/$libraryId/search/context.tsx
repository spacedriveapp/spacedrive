import { createContext, PropsWithChildren, useContext } from 'react';

import { UseSearch } from './useSearch';

const SearchContext = createContext<UseSearch<any> | null>(null);

export function useSearchContext() {
	const ctx = useContext(SearchContext);

	if (!ctx) {
		throw new Error('useSearchContext must be used within a SearchProvider');
	}

	return ctx;
}

export function SearchContextProvider({
	children,
	search
}: { search: UseSearch<any> } & PropsWithChildren) {
	return <SearchContext.Provider value={search}>{children}</SearchContext.Provider>;
}
