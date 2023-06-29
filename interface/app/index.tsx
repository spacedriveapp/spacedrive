import { useMemo } from 'react';
import { Navigate, Outlet, RouteObject, useMatches } from 'react-router-dom';
import { currentLibraryCache, useCachedLibraries, useInvalidateQuery } from '@sd/client';
import { Dialogs } from '@sd/ui';
import { RouterErrorBoundary } from '~/ErrorFallback';
import { useKeybindHandler, useTheme } from '~/hooks';
import libraryRoutes from './$libraryId';
import { RootContext } from './RootContext';
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
	useTheme();

	const rawPath = useRawRoutePath();

	return (
		<RootContext.Provider value={{ rawPath }}>
			<Outlet />
			<Dialogs />
		</RootContext.Provider>
	);
};

// NOTE: all route `Layout`s below should contain
// the `usePlausiblePageViewMonitor` hook, as early as possible (ideally within the layout itself).
// the hook should only be included if there's a valid `ClientContext` (so not onboarding)

export const routes = [
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

const useRawRoutePath = () => {
	const lastMatchId = useMatches().slice(-1)[0]!.id;

	const [rawPath] = useMemo(
		() =>
		lastMatchId
			.split('-')
			.map((s) => parseInt(s))
			.reduce(
				([rawPath, { children }], path) => {
					if (!children) return [rawPath, { children }] as any;

					const item = children[path]!;

					if (!('path' in item)) return [rawPath, item];

					return [`${rawPath}/${item.path}`, item];
				},
				['' as string, { children: routes }] as const
			),
		[lastMatchId]
	);

	return rawPath
}
