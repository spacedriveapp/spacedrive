import { RefObject, createContext, useContext, useRef } from 'react';
import { Outlet } from 'react-router';
import TopBar from '.';

interface TopBarContext {
	left: RefObject<HTMLDivElement>;
	right: RefObject<HTMLDivElement>;
}

const TopBarContext = createContext<TopBarContext | null>(null);

export const Component = () => {
	const left = useRef<HTMLDivElement>(null);
	const right = useRef<HTMLDivElement>(null);

	return (
		<TopBarContext.Provider value={{ left, right }}>
			<TopBar leftRef={left} rightRef={right} />
			<Outlet />
		</TopBarContext.Provider>
	);
};

export function useTopBarContext() {
	const ctx = useContext(TopBarContext);

	if (!ctx) throw new Error('TopBarContext not found!');

	return ctx;
}
