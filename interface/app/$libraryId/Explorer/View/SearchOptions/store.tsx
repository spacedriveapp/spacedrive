import { useEffect, useMemo } from 'react';
import { proxy, useSnapshot } from 'valtio';
import { proxyMap } from 'valtio/utils';
import { FilePathFilterArgs, ObjectFilterArgs, SearchFilterArgs } from '@sd/client';

import { FilterType, filterTypeRegistry, RenderSearchFilter } from './Filters';
import { inOrNotIn } from './util';

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
	registeredFilters: proxyMap() as Map<string, Filter>,
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
		//
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [JSON.stringify(fixedFilters)]);

	const filters = useMemo(
		() => mapFilterArgs(Array.from(store.selectedFilters.values())),
		[store.selectedFilters]
	);

	return filters;
};

export const getKey = (filter: Filter) => `${filter.type}-${filter.name}-${filter.value}`;

export const mapFilterArgs = (filters: SetFilter[]): SearchFilterArgs => {
	const args: SearchFilterArgs = {};

	filters.forEach((filter) => {
		const type = filter.type;
		const filterType = filterTypeRegistry.find((filter) => filter.name === type);
		if (filterType) filterType.apply(filter, args);
	});

	return args;
};

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
