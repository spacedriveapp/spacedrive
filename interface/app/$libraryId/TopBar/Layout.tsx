import { createContext, useContext, useState } from 'react';
import { Outlet } from 'react-router';
import { SearchFilterArgs } from '@sd/client';

import TopBar from '.';

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

	return (
		<TopBarContext.Provider value={value}>
			<TopBar />
			<Outlet />
		</TopBarContext.Provider>
	);
};

export function useTopBarContext() {
	const ctx = useContext(TopBarContext);

	if (!ctx) throw new Error('TopBarContext not found!');

	return ctx;
}
