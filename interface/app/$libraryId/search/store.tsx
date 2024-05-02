/* eslint-disable react-hooks/exhaustive-deps */
import { Icon } from '@phosphor-icons/react';
import { useEffect, useMemo } from 'react';
import { proxy, ref, useSnapshot } from 'valtio';
import { proxyMap } from 'valtio/utils';
import { SearchFilterArgs } from '@sd/client';

import { filterRegistry, FilterType, RenderSearchFilter } from './Filters';

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

const searchStore = proxy({
	interactingWithSearchOptions: false,
	searchType: 'paths' as SearchType,
	filterOptions: ref(new Map<string, FilterOptionWithType[]>()),
	// we register filters so we can search them
	registeredFilters: proxyMap() as Map<string, FilterOptionWithType>
});

// this makes the filter unique and easily searchable using .includes
export const getKey = (filter: FilterOptionWithType) =>
	`${filter.type}-${filter.name}-${filter.value}`;

// this hook allows us to register filters to the search store
// and returns the filters with the correct type
export const useRegisterSearchFilterOptions = (
	filter: RenderSearchFilter,
	options: (FilterOption & { type: FilterType })[]
) => {
	const optionsAsKeys = useMemo(() => options.map(getKey), [options]);

	useEffect(() => {
		searchStore.filterOptions.set(filter.name, options);
		searchStore.filterOptions = ref(new Map(searchStore.filterOptions));
	}, [optionsAsKeys]);

	useEffect(() => {
		const keys = options.map((option) => {
			const key = getKey(option);

			if (!searchStore.registeredFilters.has(key)) {
				searchStore.registeredFilters.set(key, option);

				return key;
			}
		});

		return () =>
			keys.forEach((key) => {
				if (key) searchStore.registeredFilters.delete(key);
			});
	}, [optionsAsKeys]);
};

export function argsToOptions(args: SearchFilterArgs[], options: Map<string, FilterOption[]>) {
	return args.flatMap((fixedArg) => {
		const filter = filterRegistry.find((f) => f.extract(fixedArg));
		if (!filter) return [];

		return filter
			.argsToOptions(filter.extract(fixedArg) as any, options)
			.map((arg) => ({ arg, filter }));
	});
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

export const resetSearchStore = () => {};

export const useSearchStore = () => useSnapshot(searchStore);

export const getSearchStore = () => searchStore;
