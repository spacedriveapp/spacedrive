import { RefObject, createContext, useRef } from 'react';
import { Outlet } from 'react-router';
import TopBar from '.';

interface TopBarContext {
	topBarChildrenRef: RefObject<HTMLDivElement> | null;
}

export const TopBarContext = createContext<TopBarContext>({
	topBarChildrenRef: null
});

export const Component = () => {
	const ref = useRef<HTMLDivElement>(null);

	return (
		<TopBarContext.Provider value={{ topBarChildrenRef: ref }}>
			<TopBar ref={ref} />
			<Outlet />
		</TopBarContext.Provider>
	);
};
