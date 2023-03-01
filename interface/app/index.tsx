import { Navigate, RouteObject, useRoutes } from 'react-router-dom';
import { currentLibraryCache, useCachedLibraries, useInvalidateQuery } from '@sd/client';
import { useKeybindHandler } from '~/hooks/useKeyboardHandler';
import { lazyEl } from '~/util';
import libraryRoutes from './$libraryId';
import onboardingRoutes from './onboarding';
import './style.scss';

const Index = () => {
	const libraries = useCachedLibraries();

	if (libraries.status !== 'success') return null;

	if (libraries.data.length === 0) return <Navigate to="onboarding" />;

	const currentLibrary = libraries.data.find((l) => l.uuid === currentLibraryCache.id);

	const libraryId = currentLibrary ? currentLibrary.uuid : libraries.data[0]?.uuid;

	return <Navigate to={`${libraryId}/overview`} />;
};


// NOTE: all route `Layout`s below should contain
// the `PlausibleTracker` component, as early as possible (ideally within the layout itself).
// if not, we will need more instances of the tracker to cover all of the routes
// which would worsen code readability and result in more `useMemo` refreshes when navigating between layouts

const routes = [
	{
		index: true,
		element: <Index />
	},
	{
		path: 'onboarding',
		element: lazyEl(() => import('./onboarding/Layout')),
		children: onboardingRoutes
	},
	{
		path: ':libraryId',
		element: lazyEl(() => import('./$libraryId/Layout')),
		children: libraryRoutes
	}
] satisfies RouteObject[];

export default () => {
	useKeybindHandler();
	useInvalidateQuery();

	return useRoutes(routes);
};
