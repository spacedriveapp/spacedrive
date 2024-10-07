/* eslint-disable react-hooks/exhaustive-deps */
import { Icon } from '@phosphor-icons/react';
import { useEffect, useMemo } from 'react';
import { proxy, ref, useSnapshot } from 'valtio';
import { proxyMap } from 'valtio/utils';
import { Range, SearchFilterArgs } from '@sd/client';

import { FilterType, RenderSearchFilter } from '.';
import { filterRegistry } from './FilterRegistry';

// TODO: this store should be in @sd/client

// Define filter option interface
export interface FilterOption<T = any> {
	name: string;
	value: string | number | Range<T> | any;
	icon?: string | Icon;
}

// Filter type is the `name` field of a filter inferred from the filter registry
export interface FilterOptionWithType extends FilterOption {
	type: FilterType;
}

const filterOptionStore = proxy({
	filterOptions: ref(new Map<string, FilterOptionWithType[]>()),
	registeredFilters: proxyMap() as Map<string, FilterOptionWithType>
});

// Generate a unique key for a filter option
export const getKey = (filter: FilterOptionWithType) =>
	`${filter.type}-${filter.name}-${filter.value}`;

// Hook to register filter options into the local store
export const useRegisterFilterOptions = (
	filter: RenderSearchFilter,
	options: (FilterOption & { type: FilterType })[]
) => {
	const optionsAsKeys = useMemo(() => options.map(getKey), [options]);

	useEffect(() => {
		filterOptionStore.filterOptions.set(filter.name, options);
		filterOptionStore.filterOptions = ref(new Map(filterOptionStore.filterOptions));
	}, [optionsAsKeys]);

	useEffect(() => {
		const keys = options.map((option) => {
			const key = getKey(option);
			if (!filterOptionStore.registeredFilters.has(key)) {
				filterOptionStore.registeredFilters.set(key, option);
				return key;
			}
		});

		return () => {
			keys.forEach((key) => {
				if (key) filterOptionStore.registeredFilters.delete(key);
			});
		};
	}, [optionsAsKeys]);
};

// Function to retrieve registered filters based on a query
export const useSearchRegisteredFilters = (query: string) => {
	const { registeredFilters } = useFilterOptionStore();

	return useMemo(() => {
		if (!query) return [];
		// Filter the registered filters by matching the query string
		return [...registeredFilters.entries()]
			.filter(([key, _]) => key.toLowerCase().includes(query.toLowerCase()))
			.map(([key, filter]) => ({ ...filter, key }));
	}, [registeredFilters, query]);
};

// Get snapshot of the filter option store
export const useFilterOptionStore = () => useSnapshot(filterOptionStore);

// Function to reset filter options (if needed)
export const resetFilterOptionStore = () => {
	filterOptionStore.filterOptions.clear();
	filterOptionStore.registeredFilters.clear();
};

// Helper to convert arguments to filter options
export function argsToFilterOptions(
	args: SearchFilterArgs[],
	options: Map<string, FilterOption[]>
) {
	return args.flatMap((fixedArg) => {
		const filter = filterRegistry.find((f) => f.extract(fixedArg));
		if (!filter) return [];

		return filter
			.argsToFilterOptions(filter.extract(fixedArg) as any, options)
			.map((arg) => ({ arg, filter }));
	});
}

export type AllKeys<T> = T extends any ? keyof T : never;
