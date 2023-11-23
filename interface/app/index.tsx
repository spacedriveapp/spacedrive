import { useMemo } from 'react';
import { Navigate, Outlet, useMatches, type RouteObject } from 'react-router-dom';
import { currentLibraryCache, useCachedLibraries } from '@sd/client';
import { Dialogs, Toaster } from '@sd/ui';
import { RouterErrorBoundary } from '~/ErrorFallback';

import libraryRoutes from './$libraryId';
import onboardingRoutes from './onboarding';
import { RootContext } from './RootContext';

import './style.scss';

import { useOperatingSystem } from '~/hooks';

import { OperatingSystem } from '..';

const Index = () => {
	const libraries = useCachedLibraries();

	// useQuery(['hasFda'], async () => {
	// 	console.log(await platform.hasFda?.());
	// 	if (os === 'macOS' && (await platform.hasFda?.()) === false) {
	// 		return navigate('/full-disk');
	// 	} else {
	// 		return null;
	// 	}
	// });

	if (libraries.status !== 'success') return null;

	if (libraries.data.length === 0) return <Navigate to="onboarding" replace />;

	const currentLibrary = libraries.data.find((l) => l.uuid === currentLibraryCache.id);

	const libraryId = currentLibrary ? currentLibrary.uuid : libraries.data[0]?.uuid;

	return <Navigate to={`${libraryId}`} replace />;
};

const Wrapper = () => {
	const rawPath = useRawRoutePath();

	return (
		<RootContext.Provider value={{ rawPath }}>
			<Outlet />
			<Dialogs />
			<Toaster position="bottom-right" expand={true} />
		</RootContext.Provider>
	);
};

// NOTE: all route `Layout`s below should contain
// the `usePlausiblePageViewMonitor` hook, as early as possible (ideally within the layout itself).
// the hook should only be included if there's a valid `ClientContext` (so not onboarding)

export const routes = (os: OperatingSystem) => {
	return [
		{
			element: <Wrapper />,
			errorElement: <RouterErrorBoundary />,
			children: [
				{
					index: true,
					element: <Index />
				},
				{
					path: 'onboarding',
					lazy: () => import('./onboarding/Layout'),
					children: onboardingRoutes(os)
				},
				{
					path: ':libraryId',
					lazy: () => import('./$libraryId/Layout'),
					children: libraryRoutes
				}
			]
		}
	] as RouteObject[];
};

/**
 * Combines the `path` segments of the current route into a single string.
 * This is useful for things like analytics, where we want the route path
 * but not the values used in the route params.
 */
const useRawRoutePath = () => {
	// `useMatches` returns a list of each matched RouteObject,
	// we grab the last one as it contains all previous route segments.
	const lastMatchId = useMatches().slice(-1)[0]?.id;
	const os = useOperatingSystem();

	const rawPath = useMemo(() => {
		const [rawPath] =
			lastMatchId
				// Gets a list of the index of each route segment
				?.split('-')
				?.map((s) => parseInt(s))
				// Gets the route object for each segment and appends the `path`, if there is one
				?.reduce(
					([rawPath, { children }], path) => {
						// No `children`, nowhere to go
						if (!children) return [rawPath, { children }] as any;

						const item = children[path]!;

						// No `path`, continue without adding to path
						if (!('path' in item)) return [rawPath, item];

						// `path` found, chuck it on the end
						return [`${rawPath}/${item.path}`, item];
					},
					['' as string, { children: routes(os) }] as const
				) ?? [];

		return rawPath ?? '/';
	}, [lastMatchId, os]);

	return rawPath;
};
