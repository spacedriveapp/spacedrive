/* eslint-disable react-hooks/exhaustive-deps */
import { useEffect, useMemo } from 'react';
import { proxy, ref, useSnapshot } from 'valtio';
import { proxyMap } from 'valtio/utils';
import { SearchFilterArgs } from '@sd/client';

import { FilterType, RenderSearchFilter } from './Filters';
import { FilterTypeCondition } from './util';

export type SearchType = 'paths' | 'objects';

export type SearchScope = 'directory' | 'location' | 'device' | 'library';

export interface FilterOption {
	value: string | any;
	name: string;
	icon?: string; // "Folder" or "#efefef"
}

export interface Filter extends FilterOption {
	type: FilterType;
}

export type AllKeys<T> = T extends any ? keyof T : never;

export interface SetFilter extends Filter {
	condition: AllKeys<FilterTypeCondition[keyof FilterTypeCondition]>;
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
	filterArgs: ref([] as SearchFilterArgs[]),
	fixedFilters: ref([] as SearchFilterArgs[]),
	filterOptions: ref(new Map<RenderSearchFilter, FilterOption[]>()),
	// we register filters so we can search them
	registeredFilters: proxyMap() as Map<string, Filter>,
	// selected filters are applied to the search args
	selectedFilters: proxyMap() as Map<string, SetFilter>
});

export const useSearchFiltersOld = <T extends SearchType>(
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
					searchStore.registeredFilters.set(getKey(filter), filter);
					selectFilterOption(filter, true, false);
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

export function useSearchFilters<T extends SearchType>(
	searchType: T,
	fixedFilters: SearchFilterArgs[]
) {
	const state = useSearchStore();

	useEffect(() => {
		resetSearchStore();
		searchStore.fixedFilters = ref(fixedFilters);
		searchStore.filterArgs = ref(fixedFilters);
	}, [fixedFilters]);

	return [...state.filterArgs];
}

// this makes the filter unique and easily searchable using .includes
export const getKey = (filter: Filter) => `${filter.type}-${filter.name}-${filter.value}`;

// this hook allows us to register filters to the search store
// and returns the filters with the correct type
export const useRegisterSearchFilterOptions = (
	filter: RenderSearchFilter,
	options: (FilterOption & { type: FilterType })[]
) => {
	useEffect(
		() => {
			if (options) {
				searchStore.filterOptions.set(filter, options);
				return () => {
					searchStore.filterOptions.delete(filter);
				};
			}
		},
		options?.map(getKey) ?? []
	);

	useEffect(
		() => {
			const keys = options?.map((filter) => {
				const key = getKey(filter);

				if (!searchStore.registeredFilters.has(key)) {
					searchStore.registeredFilters.set(key, filter);

					return key;
				}
			});

			return () =>
				keys?.forEach((key) => {
					if (key) searchStore.registeredFilters.delete(key);
				});
		},
		options?.map(getKey) ?? []
	);
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

export const selectFilterOption = (filter: Filter, condition = true, canBeRemoved = true) => {
	const key = getKey(filter);
	searchStore.selectedFilters.set(key, {
		...filter,
		condition,
		canBeRemoved
	});
};

export const deselectFilterOption = (filter: Filter) => {
	const key = getKey(filter);
	const setFilter = searchStore.selectedFilters.get(key);
	if (setFilter?.canBeRemoved !== false) searchStore.selectedFilters.delete(key);
};

export const searchRegisteredFilters = (query: string) => {
	if (!query) return [];

	return [...searchStore.registeredFilters.entries()]
		.filter(([key, _]) => {
			return key.toLowerCase().includes(query.toLowerCase());
		})
		.map(([key, filter]) => ({ ...filter, key }));
};

export const resetSearchStore = () => {
	searchStore.searchQuery = null;
	searchStore.selectedFilters.clear();
};

export const useSearchStore = () => useSnapshot(searchStore);

export const getSearchStore = () => searchStore;
