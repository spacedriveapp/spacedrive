import { proxy, useSnapshot } from 'valtio';
import { ObjectKind } from '@sd/client';

export type SearchType = 'paths' | 'objects' | 'tags';

export type SearchScope = 'directory' | 'location' | 'device' | 'library';

const searchStore = proxy({
	isSearching: false,
	interactingWithSearchOptions: false,
	searchType: 'paths',
	searchScope: 'directory',
	objectKind: null as typeof ObjectKind | null,
	tagged: null as string[] | null,
	dateRange: null as [Date, Date] | null
});

export const useSearchStore = () => useSnapshot(searchStore);

export const getSearchStore = () => searchStore;
