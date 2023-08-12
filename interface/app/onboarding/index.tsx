import { Navigate, RouteObject } from 'react-router';
import { getOnboardingStore } from '@sd/client';
import Alpha from './alpha';
import { useOnboardingContext } from './context';
import CreatingLibrary from './creating-library';
import NewLibrary from './new-library';
import Privacy from './privacy';

const Index = () => {
	const obStore = getOnboardingStore();
	const ctx = useOnboardingContext();

	// This is neat because restores the last active screen, but only if it is not the starting screen
	// Ignoring if people navigate back to the start if progress has been made
	if (obStore.lastActiveScreen && obStore.unlockedScreens.length > 1 && !ctx.library)
		return <Navigate to={obStore.lastActiveScreen} replace />;

	return <Navigate to="alpha" replace />;
};

export default [
	{
		index: true,
		element: <Index />
	},
	{ path: 'alpha', element: <Alpha /> },
	{
		element: <NewLibrary />,
		path: 'new-library'
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
