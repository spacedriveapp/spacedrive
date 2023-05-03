import { Navigate, Outlet, RouteObject } from 'react-router-dom';
import { currentLibraryCache, useCachedLibraries, useInvalidateQuery } from '@sd/client';
import { Dialogs } from '@sd/ui';
import { useKeybindHandler } from '~/hooks/useKeyboardHandler';
import libraryRoutes from './$libraryId';
import onboardingRoutes from './onboarding';
import './style.scss';

const Index = () => {
	const libraries = useCachedLibraries();

	if (libraries.status !== 'success') return null;

	if (libraries.data.length === 0) return <Navigate to="onboarding" replace />;

	const currentLibrary = libraries.data.find((l) => l.uuid === currentLibraryCache.id);

	const libraryId = currentLibrary ? currentLibrary.uuid : libraries.data[0]?.uuid;

	return <Navigate to={`${libraryId}/overview`} replace />;
};

const Wrapper = () => {
	useKeybindHandler();
	useInvalidateQuery();

	return (
		<>
			<Outlet />
			<Dialogs />
		</>
	);
};

// NOTE: all route `Layout`s below should contain
// the `usePlausiblePageViewMonitor` hook, as early as possible (ideally within the layout itself).
// the hook should only be included if there's a valid `ClientContext` (so not onboarding)

export const routes = [
	{
		element: <Wrapper />,
		children: [
			{
				index: true,
				element: <Index />
			},
			{
				path: 'onboarding',
				lazy: () => import('./onboarding/Layout'),
				children: onboardingRoutes
			},
			{
				path: ':libraryId',
				lazy: () => import('./$libraryId/Layout'),
				children: libraryRoutes
			}
		]
	}
] satisfies RouteObject[];
