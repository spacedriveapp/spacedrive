import { produce } from 'immer';
import { useCallback, useMemo, useState } from 'react';
import { useSearchParams as useRawSearchParams } from 'react-router-dom';
import { useDebouncedValue } from 'rooks';
import { SearchFilterArgs } from '@sd/client';

import { argsToOptions, getKey, useSearchStore } from './store';

export interface UseSearchSource {
	filters?: SearchFilterArgs[];
	setFilters?: (cb?: (filters?: SearchFilterArgs[]) => SearchFilterArgs[] | undefined) => void;
	search?: string;
	setSearch?: (search?: string) => void;
	open?: boolean;
}

export interface UseSearchProps {
	source: UseSearchSource;
}

export function useSearchParamsSource(): UseSearchSource {
	const [searchParams, setSearchParams] = useRawSearchParams();

	const filtersSearchParam = searchParams.get('filters');
	const filters = useMemo<SearchFilterArgs[] | undefined>(
		() => (filtersSearchParam ? JSON.parse(filtersSearchParam) : undefined),
		[filtersSearchParam]
	);

	const searchSearchParam = searchParams.get('search');

	const setFilters = useCallback(
		(cb?: (args?: SearchFilterArgs[]) => SearchFilterArgs[] | undefined) => {
			setSearchParams(
				(p) => {
					if (cb === undefined) p.delete('filters');
					else p.set('filters', JSON.stringify(produce(filters, cb)));

					return p;
				},
				{ replace: true }
			);
		},
		[filters, setSearchParams]
	);

	function setSearch(search?: string) {
		setSearchParams(
			(p) => {
				if (search && search !== '') p.set('search', search);
				else p.delete('search');

				return p;
			},
			{ replace: true }
		);
	}

	return {
		filters,
		setFilters,
		search: searchSearchParam ?? '',
		setSearch,
		open: searchSearchParam !== null || filtersSearchParam !== null
	};
}

export function useStaticSource(
	props: Pick<UseSearchSource, 'filters' | 'search'>
): UseSearchSource {
	return props;
}

export function useMemorySource(props: {
	initialFilters?: SearchFilterArgs[];
	initialSearch?: string;
}): UseSearchSource {
	const [filters, setFilters] = useState(props.initialFilters);
	const [search, setSearch] = useState(props.initialSearch);

	return {
		filters,
		setFilters: (s) => {
			if (s === undefined) setFilters(undefined);
			else setFilters((f) => produce(f, s));
		},
		search,
		setSearch
	};
}

export function useSearch(props: UseSearchProps) {
	const { filters, setFilters, search: rawSearch, setSearch, open } = props.source;

	const [searchBarFocused, setSearchBarFocused] = useState(false);

	const searchState = useSearchStore();

	const filtersAsOptions = useMemo(
		() => argsToOptions(filters ?? [], searchState.filterOptions),
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

	// Merging of filters that should be ORed

	const mergedFilters = useMemo(
		() => filters?.map((arg, removalIndex) => ({ arg, removalIndex })),
		[filters]
	);

	const [search] = useDebouncedValue(rawSearch, 300);

	const searchFilters = useMemo(() => {
		const [name, ext] = search?.split('.') ?? [];

		const filters: SearchFilterArgs[] = [];

		if (name) filters.push({ filePath: { name: { contains: name } } });
		if (ext) filters.push({ filePath: { extension: { in: [ext] } } });

		return filters;
	}, [search]);

	// All filters combined together
	const allFilters = useMemo(
		() => [...(filters ?? []), ...searchFilters],
		[filters, searchFilters]
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
		open: open || searchBarFocused,
		search,
		// rawSearch should only ever be read by the search input
		rawSearch,
		setSearch,
		searchBarFocused,
		setSearchBarFocused,
		filters,
		setFilters,
		filtersKeys,
		mergedFilters,
		allFilters,
		allFiltersKeys
	};
}

export function useSearchFromSearchParams() {
	return useSearch({
		source: useSearchParamsSource()
	});
}

export type UseSearch = ReturnType<typeof useSearch>;
