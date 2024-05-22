import { useEffect } from 'react';
import { Outlet } from 'react-router';

import TopBar from '.';
import { explorerStore } from '../Explorer/store';
import { TopBarContext, useContextValue } from './Context';

export const Component = () => {
	const value = useContextValue();

	// Reset drag state
	useEffect(() => {
		return () => {
			explorerStore.drag = null;
		};
	}, []);

	return (
		<TopBarContext.Provider value={value}>
			<TopBar />
			<Outlet />
		</TopBarContext.Provider>
	);
};
