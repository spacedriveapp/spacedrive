/* eslint-disable react-hooks/exhaustive-deps */
import { useEffect, useMemo } from 'react';
import { proxy, useSnapshot } from 'valtio';
import { proxyMap } from 'valtio/utils';
import { SearchFilterArgs } from '@sd/client';

import { FilterType, filterTypeRegistry } from './Filters';

export type SearchType = 'paths' | 'objects';

export type SearchScope = 'directory' | 'location' | 'device' | 'library';

export interface FilterArgs {
	value: string | any;
	name: string;
	icon?: string; // "Folder" or "#efefef"
}

export interface Filter extends FilterArgs {
	type: FilterType;
}

export interface SetFilter extends Filter {
	condition: boolean;
	canBeRemoved: boolean;
}

export interface GroupedFilters {
	type: FilterType;
	filters: SetFilter[];
}

const searchStore = proxy({
	isSearching: false,
	interactingWithSearchOptions: false,
	searchType: 'paths' as SearchType,
	searchQuery: null as string | null,
	// we register filters so we can search them
	registeredFilters: proxyMap() as Map<string, Filter>,
	// selected filters are applied to the search args
	selectedFilters: proxyMap() as Map<string, SetFilter>
});

export const useSearchFilters = <T extends SearchType>(
	searchType: T,
	fixedFilters?: Filter[]
): SearchFilterArgs => {
	const store = useSnapshot(searchStore);

	useEffect(() => {
		resetSearchStore();
		if (fixedFilters) {
			for (const filter of fixedFilters) {
				if (filter.name) {
					if (!filter.icon) filter.icon = filter.name;
					searchStore.registeredFilters.set(filter.value, filter);
					selectFilter(filter, true, false);
				}
			}
		}
	}, [JSON.stringify(fixedFilters)]);

	const filters = useMemo(
		() => mapFilterArgs(Array.from(store.selectedFilters.values())),
		[store.selectedFilters]
	);

	return filters;
};

// this makes the filter unique and easily searchable using .includes
export const getKey = (filter: Filter) => `${filter.type}-${filter.name}-${filter.value}`;

// this maps the filters to the search args
export const mapFilterArgs = (filters: SetFilter[]): SearchFilterArgs => {
	const args: SearchFilterArgs = {};

	filters.forEach((filter) => {
		const type = filter.type;
		const filterType = filterTypeRegistry.find((filter) => filter.name === type);
		if (filterType) filterType.apply(filter, args);
	});

	return args;
};

// this hook allows us to register filters to the search store
// and returns the filters with the correct type
export const useRegisterSearchFilterOptions = (filterType: FilterType, filters?: FilterArgs[]) => {
	return useMemo(() => {
		if (!filters) return;
		return filters.map((filterArgs) => {
			const filter = {
					...filterArgs,
					type: filterType
				},
				key = getKey(filter);
			if (searchStore.registeredFilters.has(key)) {
				searchStore.registeredFilters.set(key, filter);
			}
			return filter;
		});
		// feel free to come up with a better dependency array, for now this works
	}, [filters?.map((filter) => filter.name).join('')]);
};

// this is used to render the applied filters
export const getSelectedFiltersGrouped = (): GroupedFilters[] => {
	const groupedFilters: GroupedFilters[] = [];

	searchStore.selectedFilters.forEach((filter) => {
		const group = groupedFilters.find((group) => group.type === filter.type);
		if (group) {
			group.filters.push(filter);
		} else {
			groupedFilters.push({
				type: filter.type,
				filters: [filter]
			});
		}
	});

	return groupedFilters;
};

export const selectFilter = (filter: Filter, condition = true, canBeRemoved = true) => {
	const key = getKey(filter);
	searchStore.selectedFilters.set(key, {
		...filter,
		condition,
		canBeRemoved
	});
};

export const deselectFilter = (filter: Filter) => {
	const key = getKey(filter);
	const setFilter = searchStore.selectedFilters.get(key);
	if (setFilter?.canBeRemoved !== false) searchStore.selectedFilters.delete(key);
};

export const searchRegisteredFilters = (query: string) => {
	if (!query) return [];
	const keys = Array.from(searchStore.registeredFilters.keys()).filter(
		(filter) => filter?.toLowerCase().includes(query.toLowerCase())
	);
	return keys.map((key) => {
		const filter = searchStore.registeredFilters.get(key)!;
		return {
			...filter,
			key
		};
	});
};

export const resetSearchStore = () => {
	searchStore.searchQuery = null;
	searchStore.selectedFilters.clear();
};

export const useSearchStore = () => useSnapshot(searchStore);

export const getSearchStore = () => searchStore;
