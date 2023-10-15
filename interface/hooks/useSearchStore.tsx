import { proxy, useSnapshot } from 'valtio';

// import { ObjectKind } from '@sd/client';

export type SearchType = 'paths' | 'objects' | 'tags';

export type SearchScope = 'directory' | 'location' | 'device' | 'library';

const searchStore = proxy({
	isSearching: false,
	interactingWithSearchOptions: false,
	searchScope: 'directory',
	//
	// searchType: 'paths',
	// objectKind: null as typeof ObjectKind | null,
	// tagged: null as string[] | null,
	// dateRange: null as [Date, Date] | null

	searchableFilterItems: [] as any[], // You can replace `any` with the type of your items

	registerFilterItem: (item: any) => {
		searchStore.searchableFilterItems.push(item);
	},

	unregisterFilterItem: (itemKey: string) => {
		searchStore.searchableFilterItems = searchStore.searchableFilterItems.filter(
			(i) => i.key !== itemKey
		);
	},

	filterItems: (query: string) => {
		if (!query) return searchStore.searchableFilterItems;
		return searchStore.searchableFilterItems.filter((item) =>
			item.name.toLowerCase().includes(query.toLowerCase())
		);
	}
});

export const useSearchStore = () => useSnapshot(searchStore);

export const getSearchStore = () => searchStore;
