import {
	createContext,
	PropsWithChildren,
	useContext,
	useEffect,
	useLayoutEffect,
	useMemo
} from 'react';
import { z } from 'zod';
import { SearchFilterArgs } from '@sd/client';
import { useZodSearchParams } from '~/hooks';

import { useTopBarContext } from '../../TopBar/Layout';
import { filterRegistry } from './Filters';
import { argsToOptions, getKey, getSearchStore, updateFilterArgs, useSearchStore } from './store';

const Context = createContext<ReturnType<typeof useContextValue> | null>(null);

const SEARCH_PARAMS = z.object({
	search: z.string().optional(),
	filters: z.string().optional()
});

function useContextValue() {
	const [searchParams, setSearchParams] = useZodSearchParams(SEARCH_PARAMS);
	const searchState = useSearchStore();

	const { fixedArgs, setFixedArgs } = useTopBarContext();

	const fixedArgsAsOptions = useMemo(() => {
		return fixedArgs ? argsToOptions(fixedArgs, searchState.filterOptions) : null;
	}, [fixedArgs, searchState.filterOptions]);

	const fixedArgsKeys = useMemo(() => {
		const keys = fixedArgsAsOptions
			? new Set(
					fixedArgsAsOptions?.map(({ arg, filter }) =>
						getKey({
							type: filter.name,
							name: arg.name,
							value: arg.value
						})
					)
			  )
			: null;
		return keys;
	}, [fixedArgsAsOptions]);

	const allFilterArgs = useMemo(() => {
		if (!fixedArgs) return [];

		const value: { arg: SearchFilterArgs; removalIndex: number | null }[] = fixedArgs.map(
			(arg) => ({
				arg,
				removalIndex: null
			})
		);

		if (searchParams.filters) {
			const args: SearchFilterArgs[] = JSON.parse(searchParams.filters);

			for (const [index, arg] of args.entries()) {
				const filter = filterRegistry.find((f) => f.extract(arg));
				if (!filter) continue;

				const fixedEquivalentIndex = fixedArgs.findIndex(
					(a) => filter.extract(a) !== undefined
				);
				if (fixedEquivalentIndex !== -1) {
					const merged = filter.merge(
						filter.extract(fixedArgs[fixedEquivalentIndex]!)! as any,
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
		}

		return value;
	}, [fixedArgs, searchParams.filters]);

	useLayoutEffect(() => {
		const filters = searchParams.filters;
		if (!filters) return;

		updateFilterArgs(() => JSON.parse(filters));
	}, [searchParams.filters]);

	useEffect(() => {
		if (!searchState.filterArgs) return;

		setSearchParams(
			(p) => ({
				...p,
				filters: JSON.stringify(searchState.filterArgs)
			}),
			{ replace: true }
		);
	}, [searchState.filterArgs, setSearchParams]);

	return {
		setFixedArgs,
		fixedArgs,
		fixedArgsKeys,
		allFilterArgs,
		searchQuery: searchParams.search,
		setSearchQuery(value: string) {
			setSearchParams((p) => ({ ...p, search: value }));
		},
		clearSearchQuery() {
			setSearchParams((p) => {
				delete p.search;
				return { ...p };
			});
		},
		isSearching: searchParams.search !== undefined
	};
}

export const SearchContextProvider = ({ children }: PropsWithChildren) => {
	return <Context.Provider value={useContextValue()}>{children}</Context.Provider>;
};

export function useSearchContext() {
	const ctx = useContext(Context);

	if (!ctx) throw new Error('SearchContextProvider not found!');

	return ctx;
}
