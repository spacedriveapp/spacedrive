import { createContext, PropsWithChildren, useContext } from 'react';

import { filterRegistry } from './Filters';
import { useRegisterSearchFilterOptions } from './store';
import { UseSearch } from './useSearch';

const SearchContext = createContext<UseSearch | null>(null);

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
}: { search: UseSearch } & PropsWithChildren) {
	for (const filter of filterRegistry) {
		const options = filter
			.useOptions({ search: search.search })
			.map((o) => ({ ...o, type: filter.name }));

		// eslint-disable-next-line react-hooks/rules-of-hooks
		useRegisterSearchFilterOptions(filter, options);
	}

	return <SearchContext.Provider value={search}>{children}</SearchContext.Provider>;
}
