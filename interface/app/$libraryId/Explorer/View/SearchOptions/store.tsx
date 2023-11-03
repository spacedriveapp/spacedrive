/* eslint-disable react-hooks/exhaustive-deps */
import { Icon } from '@phosphor-icons/react';
import { produce } from 'immer';
import { useEffect, useMemo } from 'react';
import { proxy, ref, useSnapshot } from 'valtio';
import { proxyMap } from 'valtio/utils';
import { SearchFilterArgs } from '@sd/client';

import { filterRegistry, FilterType, RenderSearchFilter } from './Filters';
import { FilterTypeCondition } from './util';

export type SearchType = 'paths' | 'objects';

export type SearchScope = 'directory' | 'location' | 'device' | 'library';

export interface FilterOption {
	value: string | any;
	name: string;
	icon?: string | Icon; // "Folder" or "#efefef"
}

export interface FilterOptionWithType extends FilterOption {
	type: FilterType;
}

export type AllKeys<T> = T extends any ? keyof T : never;

export interface SetFilter extends FilterOptionWithType {
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
	filterArgsKeys: ref(new Set<string>()),
	fixedFilters: ref([] as SearchFilterArgs[]),
	fixedFilterKeys: ref(new Set<string>()),
	filterOptions: ref(new Map<string, FilterOption[]>()),
	// we register filters so we can search them
	registeredFilters: proxyMap() as Map<string, FilterOptionWithType>,
	// selected filters are applied to the search args
	selectedFilters: proxyMap() as Map<string, SetFilter>
});

export function useSearchFilters<T extends SearchType>(
	_searchType: T,
	fixedArgs: SearchFilterArgs[]
) {
	const state = useSearchStore();

	const fixedArgsAsOptions = useMemo(() => {
		return argsToOptions(fixedArgs);
	}, [fixedArgs]);

	useEffect(() => {
		resetSearchStore();

		const keys = new Set(
			fixedArgsAsOptions.map(({ arg, filter }) => {
				return getKey({
					type: filter.name,
					name: arg.name,
					value: arg.value
				});
			})
		);

		searchStore.fixedFilters = ref(fixedArgs);
		searchStore.fixedFilterKeys = ref(keys);
		searchStore.filterArgs = ref(fixedArgs);
		searchStore.filterArgsKeys = ref(keys);
	}, [fixedArgsAsOptions, fixedArgs]);

	return [...state.filterArgs];
}

// this makes the filter unique and easily searchable using .includes
export const getKey = (filter: FilterOptionWithType) =>
	`${filter.type}-${filter.name}-${filter.value}`;

// this hook allows us to register filters to the search store
// and returns the filters with the correct type
export const useRegisterSearchFilterOptions = (
	filter: RenderSearchFilter,
	options: (FilterOption & { type: FilterType })[]
) => {
	useEffect(
		() => {
			if (options) {
				searchStore.filterOptions.set(filter.name, options);
				searchStore.filterOptions = ref(new Map(searchStore.filterOptions));

				return () => {
					searchStore.filterOptions.delete(filter.name);
					searchStore.filterOptions = ref(new Map(searchStore.filterOptions));
				};
			}
		},
		options?.map(getKey) ?? []
	);

	useEffect(() => {
		const keys = options.map((filter) => {
			const key = getKey(filter);

			if (!searchStore.registeredFilters.has(key)) {
				searchStore.registeredFilters.set(key, filter);

				return key;
			}
		});

		return () =>
			keys.forEach((key) => {
				if (key) searchStore.registeredFilters.delete(key);
			});
	}, options.map(getKey));
};

export function argsToOptions(args: SearchFilterArgs[]) {
	return args.flatMap((fixedArg) => {
		const filter = filterRegistry.find((f) => f.find(fixedArg))!;

		return filter.argsToOptions(filter.find(fixedArg) as any).map((arg) => ({ arg, filter }));
	});
}

export function updateFilterArgs(fn: (args: SearchFilterArgs[]) => SearchFilterArgs[]) {
	searchStore.filterArgs = ref(produce(searchStore.filterArgs, fn));
	searchStore.filterArgsKeys = ref(
		new Set(
			argsToOptions(searchStore.filterArgs).map(({ arg, filter }) =>
				getKey({
					type: filter.name,
					name: arg.name,
					value: arg.value
				})
			)
		)
	);
}

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

export const useSearchRegisteredFilters = (query: string) => {
	const { registeredFilters } = useSearchStore();

	return useMemo(
		() =>
			!query
				? []
				: [...registeredFilters.entries()]
						.filter(([key, _]) => key.toLowerCase().includes(query.toLowerCase()))
						.map(([key, filter]) => ({ ...filter, key })),
		[registeredFilters, query]
	);
};

export const resetSearchStore = () => {
	searchStore.searchQuery = null;
	searchStore.selectedFilters.clear();
};

export const useSearchStore = () => useSnapshot(searchStore);

export const getSearchStore = () => searchStore;
