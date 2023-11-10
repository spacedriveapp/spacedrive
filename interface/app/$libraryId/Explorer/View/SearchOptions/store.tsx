/* eslint-disable react-hooks/exhaustive-deps */
import { Icon } from '@phosphor-icons/react';
import { produce } from 'immer';
import { useEffect, useLayoutEffect, useMemo } from 'react';
import { proxy, ref, useSnapshot } from 'valtio';
import { proxyMap } from 'valtio/utils';
import { SearchFilterArgs } from '@sd/client';
import { useTopBarContext } from '~/app/$libraryId/TopBar/Layout';

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
	filterOptions: ref(new Map<string, FilterOption[]>()),
	// we register filters so we can search them
	registeredFilters: proxyMap() as Map<string, FilterOptionWithType>
});

export function useSearchFilters<T extends SearchType>(
	_searchType: T,
	fixedArgs: SearchFilterArgs[]
) {
	const { filterArgs } = useSearchStore();
	const topBar = useTopBarContext();

	// don't want the search bar to pop in after the top bar has loaded!
	useLayoutEffect(() => {
		topBar.setFixedArgs(fixedArgs);
	}, [fixedArgs]);

	return fixedArgs;

	// return [...state.filterArgs];
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

export function argsToOptions(args: SearchFilterArgs[], options: Map<string, FilterOption[]>) {
	return args.flatMap((fixedArg) => {
		const filter = filterRegistry.find((f) => f.extract(fixedArg))!;

		return filter
			.argsToOptions(filter.extract(fixedArg) as any, options)
			.map((arg) => ({ arg, filter }));
	});
}

export function updateFilterArgs(fn: (args: SearchFilterArgs[]) => SearchFilterArgs[]) {
	searchStore.filterArgs = ref(produce(searchStore.filterArgs, fn));
	searchStore.filterArgsKeys = ref(
		new Set(
			argsToOptions(searchStore.filterArgs, searchStore.filterOptions).map(
				({ arg, filter }) =>
					getKey({
						type: filter.name,
						name: arg.name,
						value: arg.value
					})
			)
		)
	);
}

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
	searchStore.filterArgs = ref([]);
};

export const useSearchStore = () => useSnapshot(searchStore);

export const getSearchStore = () => searchStore;
