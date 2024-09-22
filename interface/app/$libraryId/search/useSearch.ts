import { produce } from 'immer';
import { useCallback, useMemo, useState } from 'react';
import { useSearchParams as useRawSearchParams } from 'react-router-dom';
import { useDebouncedValue } from 'rooks';
import { SearchFilterArgs } from '@sd/client';

import { argsToFilterOptions, getKey, useFilterOptionStore } from './Filters/store';

export type SearchTarget = 'paths' | 'objects';

export interface UseSearchSource {
	target: SearchTarget;
	setTarget?: (target?: SearchTarget) => void;
	filters?: SearchFilterArgs[];
	setFilters?: (cb?: (filters?: SearchFilterArgs[]) => SearchFilterArgs[] | undefined) => void;
	search?: string;
	setSearch?: (search?: string) => void;
	open?: boolean;
}

export interface UseSearchProps<TSource extends UseSearchSource> {
	source: TSource;
}

export function useSearchParamsSource(props: { defaultTarget: SearchTarget }) {
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

	const target = (searchParams.get('target') as SearchTarget | null) ?? props.defaultTarget;

	return {
		filters,
		setFilters,
		search: searchSearchParam ?? '',
		setSearch,
		open: searchSearchParam !== null || filtersSearchParam !== null,
		target,
		setTarget: (target) =>
			setSearchParams(
				(p) => {
					if (target) p.set('target', target);
					else p.delete('target');
					return p;
				},
				{ replace: true }
			)
	} satisfies UseSearchSource;
}

export function useStaticSource(props: Pick<UseSearchSource, 'filters' | 'search' | 'target'>) {
	return props satisfies UseSearchSource;
}

export function useMemorySource(props: {
	initialFilters?: SearchFilterArgs[];
	initialSearch?: string;
	initialTarget?: SearchTarget;
}) {
	const [filters, setFilters] = useState(props.initialFilters);
	const [search, setSearch] = useState(props.initialSearch);
	const [target, setTarget] = useState(props.initialTarget ?? 'paths');

	return {
		filters,
		setFilters: (s) => {
			if (s === undefined) setFilters(undefined);
			else setFilters((f) => produce(f, s));
		},
		search,
		setSearch,
		target,
		setTarget: (t) => setTarget(t ?? 'paths')
	} satisfies UseSearchSource;
}

export function useSearch<TSource extends UseSearchSource>(props: UseSearchProps<TSource>) {
	const {
		filters,
		setFilters,
		search: rawSearch,
		setSearch,
		open,
		target,
		setTarget
	} = props.source;

	const [searchBarFocused, setSearchBarFocused] = useState(false);

	const filterStore = useFilterOptionStore();

	const filtersAsOptions = useMemo(
		() => argsToFilterOptions(filters ?? [], filterStore.filterOptions),
		[filters, filterStore.filterOptions]
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
		() => argsToFilterOptions(allFilters, filterStore.filterOptions),
		[filterStore.filterOptions, allFilters]
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
		allFiltersKeys,
		target,
		setTarget
	};
}

export function useSearchFromSearchParams(props: { defaultTarget: SearchTarget }) {
	return useSearch({
		source: useSearchParamsSource(props)
	});
}

export type UseSearch<TSource extends UseSearchSource> = ReturnType<typeof useSearch<TSource>>;
