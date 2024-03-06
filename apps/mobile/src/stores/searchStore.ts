import { proxy, useSnapshot } from 'valtio';

/**
 This is subject to change, but the idea is to have a global store for search
 that can be used by any component.

 once the new mobile design is implemented this is likely to change
 */

export type SearchFilters = "locations" | "tags" | "name" | "extension" | "hidden" | "kind"

 const state: {
	search: string;
	filters: Record<SearchFilters, any[]>;
	disableActionButtons: boolean;
 } = {
	search: '', // The search input
	filters: { // The filters in the filters page of search
		locations: [],
		tags: [],
		name: [''],
		extension: [''],
		hidden: [],
		kind: []
	},
	disableActionButtons: true //save search and add filters button
 }

const searchStore = proxy({
	...state,
	updateFilters: (filter: keyof typeof state['filters'], value: any) => {
		const updatedFilters = [...searchStore.filters[filter]]; // Make a copy of the existing filter values
		if (updatedFilters.includes(value)) {
		  // Remove the value if it already exists in the filter
		  searchStore.filters[filter] = updatedFilters.filter((v) => v !== value);
		} else {
		  // Add the value to the filter if it doesn't exist
		  searchStore.filters = {
			...searchStore.filters,
			[filter]: [...updatedFilters, value]
		  };
		}
	},
	setSearch: (search: string) => {
		searchStore.search = search;
	},
	// General filter functions
	isFilterSelected: (filter: keyof typeof state['filters'], value: string | number) => {
		return searchStore.filters[filter].includes(value);
	},
	resetFilter: (filter: keyof typeof state['filters'], isInput?: boolean) => {
		if (isInput) {
			searchStore.filters[filter] = [''];
		}
		else searchStore.filters[filter] = [];
	},
	resetFilters: () => {
		for (const filter in searchStore.filters) {
			if (filter === 'name' || filter === 'extension') {
				searchStore.filters[filter] = [''];
				continue;
			}
			searchStore.filters[filter as SearchFilters] = [];
		}
	},
	// Handling name and extension filter inputs
	setInput: (index: number, value: string, key: 'name' | 'extension') => {
		searchStore.filters[key][index] = value;
	},
	addInput: (key: 'name' | 'extension') => {
		searchStore.filters[key].push('');
	},
	removeInput: (index: number, key: 'name' | 'extension') => {
		searchStore.filters[key].splice(index, 1);
	}
});

export function useSearchStore() {
	return useSnapshot(searchStore);
}

export function getSearchStore() {
	return searchStore;
}
