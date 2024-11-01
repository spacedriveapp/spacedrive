import { Navigate, redirect, RouteObject } from 'react-router';
import { onboardingStore } from '@sd/client';

import { useOnboardingContext } from './context';
import CreatingLibrary from './creating-library';
import { FullDisk } from './full-disk';
import Locations from './locations';
import NewLibrary from './new-library';
import PreRelease from './prerelease';
import Privacy from './privacy';

const Index = () => {
	const ctx = useOnboardingContext();

	if (onboardingStore.lastActiveScreen && !ctx.library)
		return <Navigate to={onboardingStore.lastActiveScreen} replace />;

	return <Navigate to="prerelease" replace />;
};

export default [
	{
		index: true,
		loader: () => {
			if (onboardingStore.lastActiveScreen)
				return redirect(`/onboarding/${onboardingStore.lastActiveScreen}`, {
					replace: true
				});

			return redirect(`/onboarding/prerelease`, { replace: true });
		},
		element: <Index />
	},
	{ Component: PreRelease, path: 'prerelease' },
	// {
	// 	element: <Login />,
	// 	path: 'login'
	// },
	{ Component: NewLibrary, path: 'new-library' },
	{ Component: FullDisk, path: 'full-disk' },
	{ Component: Locations, path: 'locations' },
	{ Component: Privacy, path: 'privacy' },
	{ Component: CreatingLibrary, path: 'creating-library' }
] satisfies RouteObject[];
