import { produce } from 'immer';
import { useCallback, useMemo, useState } from 'react';
import { useDebouncedValue } from 'rooks';
import { SearchFilterArgs } from '@sd/client';

import { filterRegistry } from './Filters';
import { argsToOptions, getKey, useSearchStore } from './store';

export interface UseSearchProps {
	open?: boolean;
	search?: string;
	/**
	 * Filters that can be removed.
	 * When this value changes dynamic filters stored internally will reset.
	 */
	filters?: SearchFilterArgs[];
	defaultFilters?: SearchFilterArgs[];
}

export function useSearch(props?: UseSearchProps) {
	const [searchBarFocused, setSearchBarFocused] = useState(false);

	const searchState = useSearchStore();

	const [filters, setFilters] = useState(props?.filters ?? []);
	const [filtersFromProps, setFiltersFromProps] = useState(props?.filters);

	if (filtersFromProps !== props?.filters) {
		setFiltersFromProps(props?.filters);
		setFilters(props?.filters ?? []);
	}

	const filtersAsOptions = useMemo(
		() => argsToOptions(filters, searchState.filterOptions),
		[filters, searchState.filterOptions]
	);

	const filtersKeys: Set<string> = useMemo(() => {
		return new Set(
			filtersAsOptions.map(({ arg, filter }) =>
				getKey({
					type: filter.name,
					name: arg.name,
					value: arg.value
				})
			)
		);
	}, [filtersAsOptions]);

	const updateFilters = useCallback(
		(cb: (args: SearchFilterArgs[]) => SearchFilterArgs[]) =>
			setFilters((filters) => produce(filters, cb)),
		[]
	);

	// Merging of filters that should be ORed

	const mergedFilters = useMemo(() => {
		const value: { arg: SearchFilterArgs; removalIndex: number | null }[] = [];

		for (const [index, arg] of filters.entries()) {
			const filter = filterRegistry.find((f) => f.extract(arg));
			if (!filter) continue;

			value.push({
				arg,
				removalIndex: index
			});
		}

		return value;
	}, [filters]);

	// Filters generated from the search query

	// rawSearch should only ever be read by the search input
	const [rawSearch, setRawSearch] = useState(props?.search ?? '');
	const [searchFromProps, setSearchFromProps] = useState(props?.search);

	if (searchFromProps !== props?.search) {
		setSearchFromProps(props?.search);
		setRawSearch(props?.search ?? '');
	}

	const [search] = useDebouncedValue(rawSearch, 300);

	const searchFilters = useMemo(() => {
		const [name, ext] = search.split('.') ?? [];

		const filters: SearchFilterArgs[] = [];

		if (name) filters.push({ filePath: { name: { contains: name } } });
		if (ext) filters.push({ filePath: { extension: { in: [ext] } } });

		return filters;
	}, [search]);

	// All filters combined together
	const allFilters = useMemo(
		() => [...mergedFilters.map((v) => v.arg), ...searchFilters],
		[mergedFilters, searchFilters]
	);

	const allFiltersAsOptions = useMemo(
		() => argsToOptions(allFilters, searchState.filterOptions),
		[searchState.filterOptions, allFilters]
	);

	const allFiltersKeys: Set<string> = useMemo(() => {
		return new Set(
			allFiltersAsOptions.map(({ arg, filter }) =>
				getKey({
					type: filter.name,
					name: arg.name,
					value: arg.value
				})
			)
		);
	}, [allFiltersAsOptions]);

	return {
		open: props?.open || searchBarFocused,
		search,
		rawSearch,
		setSearch: setRawSearch,
		searchBarFocused,
		setSearchBarFocused,
		defaultFilters: props?.defaultFilters,
		filters,
		setFilters,
		updateFilters,
		filtersKeys,
		mergedFilters,
		allFilters,
		allFiltersKeys
	};
}

export type UseSearch = ReturnType<typeof useSearch>;
