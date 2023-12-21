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
	 * Filters that cannot be removed
	 */
	fixedFilters?: SearchFilterArgs[];
	/**
	 * Filters that can be removed.
	 * When this value changes dynamic filters stored internally will reset.
	 */
	dynamicFilters?: SearchFilterArgs[];
}

export function useSearch(props?: UseSearchProps) {
	const [searchBarFocused, setSearchBarFocused] = useState(false);

	const searchState = useSearchStore();

	// Filters that can't be removed

	const fixedFilters = useMemo(() => props?.fixedFilters ?? [], [props?.fixedFilters]);

	const fixedFiltersAsOptions = useMemo(
		() => argsToOptions(fixedFilters, searchState.filterOptions),
		[fixedFilters, searchState.filterOptions]
	);

	const fixedFiltersKeys: Set<string> = useMemo(() => {
		return new Set(
			fixedFiltersAsOptions.map(({ arg, filter }) =>
				getKey({
					type: filter.name,
					name: arg.name,
					value: arg.value
				})
			)
		);
	}, [fixedFiltersAsOptions]);

	// Filters that can be removed

	const [dynamicFilters, setDynamicFilters] = useState(props?.dynamicFilters ?? []);
	const [dynamicFiltersFromProps, setDynamicFiltersFromProps] = useState(props?.dynamicFilters);

	if (dynamicFiltersFromProps !== props?.dynamicFilters) {
		setDynamicFiltersFromProps(props?.dynamicFilters);
		setDynamicFilters(props?.dynamicFilters ?? []);
	}

	const dynamicFiltersAsOptions = useMemo(
		() => argsToOptions(dynamicFilters, searchState.filterOptions),
		[dynamicFilters, searchState.filterOptions]
	);

	const dynamicFiltersKeys: Set<string> = useMemo(() => {
		return new Set(
			dynamicFiltersAsOptions.map(({ arg, filter }) =>
				getKey({
					type: filter.name,
					name: arg.name,
					value: arg.value
				})
			)
		);
	}, [dynamicFiltersAsOptions]);

	const updateDynamicFilters = useCallback(
		(cb: (args: SearchFilterArgs[]) => SearchFilterArgs[]) =>
			setDynamicFilters((filters) => produce(filters, cb)),
		[]
	);

	// Merging of filters that should be ORed

	const mergedFilters = useMemo(() => {
		const value: { arg: SearchFilterArgs; removalIndex: number | null }[] = fixedFilters.map(
			(arg) => ({
				arg,
				removalIndex: null
			})
		);

		for (const [index, arg] of dynamicFilters.entries()) {
			const filter = filterRegistry.find((f) => f.extract(arg));
			if (!filter) continue;

			const fixedEquivalentIndex = fixedFilters.findIndex(
				(a) => filter.extract(a) !== undefined
			);

			if (fixedEquivalentIndex !== -1) {
				const merged = filter.merge(
					filter.extract(fixedFilters[fixedEquivalentIndex]!)! as any,
					filter.extract(arg)! as any
				);

				value[fixedEquivalentIndex] = {
					arg: filter.create(merged),
					removalIndex: fixedEquivalentIndex
				};
			} else {
				value.push({
					arg,
					removalIndex: index
				});
			}
		}

		return value;
	}, [fixedFilters, dynamicFilters]);

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
		fixedFilters,
		fixedFiltersKeys,
		search,
		rawSearch,
		setSearch: setRawSearch,
		searchBarFocused,
		setSearchBarFocused,
		dynamicFilters,
		setDynamicFilters,
		updateDynamicFilters,
		dynamicFiltersKeys,
		mergedFilters,
		allFilters,
		allFiltersKeys
	};
}

export type UseSearch = ReturnType<typeof useSearch>;
