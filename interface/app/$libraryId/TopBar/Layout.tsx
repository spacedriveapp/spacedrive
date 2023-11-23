import { createContext, useContext, useEffect, useState } from 'react';
import { Outlet } from 'react-router';
import { SearchFilterArgs } from '@sd/client';

import TopBar from '.';
import { SearchContextProvider } from '../Explorer/Search/Context';
import { getExplorerStore } from '../Explorer/store';

const TopBarContext = createContext<ReturnType<typeof useContextValue> | null>(null);

function useContextValue() {
	const [left, setLeft] = useState<HTMLDivElement | null>(null);
	const [right, setRight] = useState<HTMLDivElement | null>(null);
	const [fixedArgs, setFixedArgs] = useState<SearchFilterArgs[] | null>(null);
	const [topBarHeight, setTopBarHeight] = useState(0);

	return {
		left,
		setLeft,
		right,
		setRight,
		fixedArgs,
		setFixedArgs,
		topBarHeight,
		setTopBarHeight
	};
}

export const Component = () => {
	const value = useContextValue();

	// Reset drag state
	useEffect(() => {
		return () => {
			getExplorerStore().drag = null;
		};
	}, []);

	return (
		<TopBarContext.Provider value={value}>
			<SearchContextProvider>
				<TopBar />
				<Outlet />
			</SearchContextProvider>
		</TopBarContext.Provider>
	);
};

export function useTopBarContext() {
	const ctx = useContext(TopBarContext);

	if (!ctx) throw new Error('TopBarContext not found!');

	return ctx;
}
