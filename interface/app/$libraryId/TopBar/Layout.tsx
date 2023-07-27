import { RefObject, createContext, useContext, useRef, useState } from 'react';
import { Outlet } from 'react-router';
import TopBar from '.';

interface TopBarContext {
	left: RefObject<HTMLDivElement>;
	right: RefObject<HTMLDivElement>;
	setNoSearch: (value: boolean) => void;
}

const TopBarContext = createContext<TopBarContext | null>(null);

export const Component = () => {
	const left = useRef<HTMLDivElement>(null);
	const right = useRef<HTMLDivElement>(null);
	const [noSearch, setNoSearch] = useState(false);

	return (
		<TopBarContext.Provider value={{ left, right, setNoSearch }}>
			<TopBar leftRef={left} rightRef={right} noSearch={noSearch} />
			<Outlet />
		</TopBarContext.Provider>
	);
};

export function useTopBarContext() {
	const ctx = useContext(TopBarContext);

	if (!ctx) throw new Error('TopBarContext not found!');

	return ctx;
}
