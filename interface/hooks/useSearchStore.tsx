// import { Icon } from '@phosphor-icons/react';
// import { IconTypes } from '@sd/assets/util';
// import { useEffect, useState } from 'react';
// import { proxy, useSnapshot } from 'valtio';
// import { proxyMap } from 'valtio/utils';

// // import { ObjectKind } from '@sd/client';

// type SearchType = 'paths' | 'objects' | 'tags';

// type SearchScope = 'directory' | 'location' | 'device' | 'library';

// interface FilterCategory {
// 	icon: string; // must be string
// 	name: string;
// }

// /// Filters are stored in a map, so they can be accessed by key
// export interface Filter {
// 	id: string | number;
// 	icon: Icon | IconTypes | string;
// 	name: string;
// }

// // Once a filter is registered, it is given a key and a category name
// export interface RegisteredFilter extends Filter {
// 	categoryName: string; // used to link filters to category
// 	key: string; // used to identify filters in the map
// }

// // Once a filter is selected, condition state is tracked
// export interface SetFilter extends RegisteredFilter {
// 	condition: boolean;
// 	category?: FilterCategory;
// }

// interface Filters {
// 	name: string;
// 	icon: string;
// 	filters: Filter[];
// }

// export type GroupedFilters = {
// 	[categoryName: string]: SetFilter[];
// };

// export function useCreateSearchFilter({ filters, name, icon }: Filters) {
// 	const [registeredFilters, setRegisteredFilters] = useState<RegisteredFilter[]>([]);

// 	useEffect(() => {
// 		const newRegisteredFilters: RegisteredFilter[] = [];

// 		searchStore.filterCategories.set(name, { name, icon });

// 		filters.map((filter) => {
// 			const registeredFilter = searchStore.registerFilter(
// 				// id doesn't have to be a particular format, just needs to be unique
// 				`${filter.id}-${filter.name}`,
// 				filter,
// 				name
// 			);
// 			newRegisteredFilters.push(registeredFilter);
// 		});

// 		setRegisteredFilters(newRegisteredFilters);

// 		console.log(getSearchStore());

// 		return () => {
// 			filters.forEach((filter) => {
// 				searchStore.unregisterFilter(`${filter.id}-${filter.name}`);
// 			});
// 			setRegisteredFilters([]); // or filter out the unregistered filters
// 		};
// 	}, []);

// 	return {
// 		name,
// 		icon,
// 		filters: registeredFilters // returning the registered filters with their keys
// 	};
// }

// const searchStore = proxy({
// 	isSearching: false,
// 	interactingWithSearchOptions: false,
// 	searchScope: 'directory',
// 	//
// 	// searchType: 'paths',
// 	// objectKind: null as typeof ObjectKind | null,
// 	// tagged: null as string[] | null,
// 	// dateRange: null as [Date, Date] | null

// 	filters: proxyMap() as Map<string, RegisteredFilter>,
// 	filterCategories: proxyMap() as Map<string, FilterCategory>,
// 	selectedFilters: proxyMap() as Map<string, SetFilter>,

// 	registerFilter: (key: string, filter: Filter, categoryName: string) => {
// 		searchStore.filters.set(key, { ...filter, key, categoryName });
// 		return searchStore.filters.get(key)!;
// 	},

// 	unregisterFilter: (key: string) => {
// 		searchStore.filters.delete(key);
// 	},

// 	selectFilter: (key: string, condition: boolean) => {
// 		searchStore.selectedFilters.set(key, { ...searchStore.filters.get(key)!, condition });
// 	},

// 	deselectFilter: (key: string) => {
// 		searchStore.selectedFilters.delete(key);
// 	},

// 	clearSelectedFilters: () => {
// 		searchStore.selectedFilters.clear();
// 	},

// 	getSelectedFilters: (): GroupedFilters => {
// 		return Array.from(searchStore.selectedFilters.values())
// 			.map((filter) => ({
// 				...filter,
// 				category: searchStore.filterCategories.get(filter.categoryName)!
// 			}))
// 			.reduce((grouped, filter) => {
// 				if (!grouped[filter.categoryName]) {
// 					grouped[filter.categoryName] = [];
// 				}
// 				grouped[filter.categoryName]?.push(filter);
// 				return grouped;
// 			}, {} as GroupedFilters);
// 	},

// 	searchFilters: (query: string) => {
// 		if (!query) return searchStore.filters;
// 		return Array.from(searchStore.filters.values()).filter((filter) =>
// 			filter.name.toLowerCase().includes(query.toLowerCase())
// 		);
// 	},

// 	reset() {
// 		searchStore.searchScope = 'directory';
// 		searchStore.filters.clear();
// 		searchStore.filterCategories.clear();
// 		searchStore.selectedFilters.clear();
// 	}
// });

// export const useSearchStore = () => useSnapshot(searchStore);

// export const getSearchStore = () => searchStore;
