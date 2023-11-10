import { createContext, useContext, useMemo, useState } from 'react';
import { Outlet } from 'react-router';
import { SearchFilterArgs } from '@sd/client';

import TopBar from '.';
import { argsToOptions, getKey, useSearchStore } from '../Explorer/View/SearchOptions/store';

const TopBarContext = createContext<ReturnType<typeof useContextValue> | null>(null);

function useContextValue(props: { left: HTMLDivElement | null; right: HTMLDivElement | null }) {
	const [fixedArgs, setFixedArgs] = useState<SearchFilterArgs[] | null>(null);

	const searchState = useSearchStore();

	const fixedArgsAsOptions = useMemo(() => {
		return fixedArgs ? argsToOptions(fixedArgs, searchState.filterOptions) : null;
	}, [fixedArgs, searchState.filterOptions]);

	const fixedArgsKeys = useMemo(() => {
		const keys = fixedArgsAsOptions
			? new Set(
					fixedArgsAsOptions?.map(({ arg, filter }) => {
						return getKey({
							type: filter.name,
							name: arg.name,
							value: arg.value
						});
					})
			  )
			: null;
		return keys;
	}, [fixedArgsAsOptions]);

	return { ...props, setFixedArgs, fixedArgs, fixedArgsKeys };
}

export const Component = () => {
	const [left, setLeft] = useState<HTMLDivElement | null>(null);
	const [right, setRight] = useState<HTMLDivElement | null>(null);

	return (
		<TopBarContext.Provider value={useContextValue({ left, right })}>
			<TopBar leftRef={setLeft} rightRef={setRight} />
			<Outlet />
		</TopBarContext.Provider>
	);
};

export function useTopBarContext() {
	const ctx = useContext(TopBarContext);

	if (!ctx) throw new Error('TopBarContext not found!');

	return ctx;
}
