import { createContext, useContext } from 'react';

import { UseSearch } from './useSearch';

export const SearchContext = createContext<UseSearch | null>(null);

export function useSearchContext() {
	const ctx = useContext(SearchContext);

	if (!ctx) {
		throw new Error('useSearchContext must be used within a SearchProvider');
	}

	return ctx;
}
