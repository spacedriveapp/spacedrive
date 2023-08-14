import { createContext, useContext, useState } from 'react';
import { Outlet } from 'react-router';
import TopBar from '.';

interface TopBarContext {
	left: HTMLDivElement | null;
	right: HTMLDivElement | null;
	setNoSearch: (value: boolean) => void;
}

const TopBarContext = createContext<TopBarContext | null>(null);

export const Component = () => {
	const [left, setLeft] = useState<HTMLDivElement | null>(null);
	const [right, setRight] = useState<HTMLDivElement | null>(null);
	const [noSearch, setNoSearch] = useState(false);

	return (
		<TopBarContext.Provider value={{ left, right, setNoSearch }}>
			<TopBar leftRef={setLeft} rightRef={setRight} noSearch={noSearch} />
			<Outlet />
		</TopBarContext.Provider>
	);
};

export function useTopBarContext() {
	const ctx = useContext(TopBarContext);

	if (!ctx) throw new Error('TopBarContext not found!');

	return ctx;
}
