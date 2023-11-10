import { createContext, useContext, useState } from 'react';
import { Outlet } from 'react-router';
import { SearchFilterArgs } from '@sd/client';

import TopBar from '.';

const TopBarContext = createContext<ReturnType<typeof useContextValue> | null>(null);

function useContextValue(props: { left: HTMLDivElement | null; right: HTMLDivElement | null }) {
	const [fixedArgs, setFixedArgs] = useState<SearchFilterArgs[] | null>(null);

	return { ...props, fixedArgs, setFixedArgs };
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
