import { Navigate, RouteObject } from 'react-router';
import { getOnboardingStore } from '@sd/client';

import Alpha from './alpha';
import { useOnboardingContext } from './context';
import CreatingLibrary from './creating-library';
import Locations from './locations';
import NewLibrary from './new-library';
import Privacy from './privacy';

const Index = () => {
	const obStore = getOnboardingStore();
	const ctx = useOnboardingContext();

	if (obStore.lastActiveScreen && !ctx.library)
		return <Navigate to={obStore.lastActiveScreen} replace />;

	return <Navigate to="alpha" replace />;
};

export default [
	{
		index: true,
		element: <Index />
	},
	{ path: 'alpha', element: <Alpha /> },
	// {
	// 	element: <Login />,
	// 	path: 'login'
	// },
	{
		element: <NewLibrary />,
		path: 'new-library'
	},
	{
		element: <Locations />,
		path: 'locations'
	},
	{
		element: <Privacy />,
		path: 'privacy'
	},
	{
		element: <CreatingLibrary />,
		path: 'creating-library'
	}
] satisfies RouteObject[];
