import { Icon } from '@phosphor-icons/react';
import { IconTypes } from '@sd/assets/util';
import { useEffect } from 'react';
import { proxy, useSnapshot } from 'valtio';

// import { ObjectKind } from '@sd/client';

export type SearchType = 'paths' | 'objects' | 'tags';

export type SearchScope = 'directory' | 'location' | 'device' | 'library';

export type SelectedFilter = {
	condition: boolean;
	key: string;
};

const searchStore = proxy({
	isSearching: false,
	interactingWithSearchOptions: false,
	searchScope: 'directory',
	//
	// searchType: 'paths',
	// objectKind: null as typeof ObjectKind | null,
	// tagged: null as string[] | null,
	// dateRange: null as [Date, Date] | null

	searchableFilterItems: {} as Record<string, SearchOptionMenu>, // You can replace `any` with the type of your items
	selectedFilters: {} as Record<string, SelectedFilter>,

	registerFilterItem: (itemKey: string, item: SearchOptionMenu) => {
		searchStore.searchableFilterItems[itemKey] = item;
		console.log(searchStore.searchableFilterItems);
	},

	unregisterFilterItem: (itemKey: string) => {
		delete searchStore.searchableFilterItems[itemKey];
	},

	selectFilter: (itemKey: string, condition: boolean) => {
		searchStore.selectedFilters[itemKey] = { key: itemKey, condition };
	},

	hasFilter: (itemKey: string) => {
		return !!searchStore.selectedFilters[itemKey];
	},

	deselectFilter: (itemKey: string) => {
		delete searchStore.selectedFilters[itemKey];
	},

	clearSelectedFilters: () => {
		searchStore.selectedFilters = {};
	},

	getSelectedFilters: () => {
		return Object.values(searchStore.selectedFilters);
	}

	// filterItems: (query: string) => {
	// 	if (!query) return searchStore.searchableFilterItems;
	// 	return searchStore.searchableFilterItems.filter((item) =>
	// 		item.name.toLowerCase().includes(query.toLowerCase())
	// 	);
	// }
});

export const useSearchStore = () => useSnapshot(searchStore);

export const getSearchStore = () => searchStore;

export interface SearchOptionItem {
	key?: string;
	name: string;
	icon?: Icon | IconTypes;
}

export interface SearchOptionMenu extends SearchOptionItem {
	options: SearchOptionItem[];
}

export function useSearchOption(item: SearchOptionMenu) {
	useEffect(() => {
		if (item.key) searchStore.registerFilterItem(item.key, item);
		return () => {
			if (item.key) searchStore.unregisterFilterItem(item.key);
		};
	}, []);
	return item;
}
