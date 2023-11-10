import { createContext, PropsWithChildren, useContext, useMemo } from 'react';

import { useTopBarContext } from '../../TopBar/Layout';
import { argsToOptions, getKey, useSearchStore } from './store';

const Context = createContext<ReturnType<typeof useContextValue> | null>(null);

function useContextValue() {
	const searchState = useSearchStore();

	const { fixedArgs, setFixedArgs } = useTopBarContext();

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

	return { setFixedArgs, fixedArgs, fixedArgsKeys };
}

export const SearchContextProvider = ({ children }: PropsWithChildren) => {
	return <Context.Provider value={useContextValue()}>{children}</Context.Provider>;
};

export function useSearchContext() {
	const ctx = useContext(Context);

	if (!ctx) throw new Error('SearchContextProvider not found!');

	return ctx;
}
