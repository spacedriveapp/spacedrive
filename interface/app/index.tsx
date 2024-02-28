import { initRspc, wsBatchLink, type AlphaClient } from '@oscartbeaumont-sd/rspc-client/v2';
import { useEffect, useMemo, useState } from 'react';
import { Navigate, Outlet, redirect, useMatches, type RouteObject } from 'react-router-dom';
import {
	context,
	currentLibraryCache,
	getCachedLibraries,
	LibraryContextProvider,
	nonLibraryClient,
	NormalisedCache,
	Procedures,
	useCachedLibraries,
	useFeatureFlag,
	useLibraryContext,
	useNormalisedCache,
	useRspcContext,
	WithSolid
} from '@sd/client';
import { Dialogs, Toaster, z } from '@sd/ui';
import { RouterErrorBoundary } from '~/ErrorFallback';
import { useRoutingContext } from '~/RoutingContext';

import { Platform, usePlatform } from '..';
import libraryRoutes from './$libraryId';
import { DragAndDropDebug } from './$libraryId/debug/dnd';
import { Demo, Demo2 } from './demo.solid';
import onboardingRoutes from './onboarding';
import { RootContext } from './RootContext';

import './style.scss';

import { useZodRouteParams } from '~/hooks';

// NOTE: all route `Layout`s below should contain
// the `usePlausiblePageViewMonitor` hook, as early as possible (ideally within the layout itself).
// the hook should only be included if there's a valid `ClientContext` (so not onboarding)

export const createRoutes = (platform: Platform, cache: NormalisedCache) =>
	[
		{
			Component: () => {
				const rawPath = useRawRoutePath();

				return (
					<RootContext.Provider value={{ rawPath }}>
						{useFeatureFlag('debugDragAndDrop') ? <DragAndDropDebug /> : null}
						{useFeatureFlag('solidJsDemo') ? (
							<WithSolid root={Demo} demo="123" />
						) : null}
						{useFeatureFlag('solidJsDemo') ? <WithSolid root={Demo2} /> : null}
						<Outlet />
						<Dialogs />
						<Toaster position="bottom-right" expand={true} offset={18} />
					</RootContext.Provider>
				);
			},
			errorElement: <RouterErrorBoundary />,
			children: [
				{
					index: true,
					Component: () => {
						const libraries = useCachedLibraries();

						if (libraries.status !== 'success') return null;

						if (libraries.data.length === 0)
							return <Navigate to="onboarding" replace />;

						const currentLibrary = libraries.data.find(
							(l) => l.uuid === currentLibraryCache.id
						);

						const libraryId = currentLibrary
							? currentLibrary.uuid
							: libraries.data[0]?.uuid;

						return <Navigate to={`${libraryId}`} replace />;
					},
					loader: async () => {
						const libraries = await getCachedLibraries(cache, nonLibraryClient);

						const currentLibrary = libraries.find(
							(l) => l.uuid === currentLibraryCache.id
						);

						const libraryId = currentLibrary ? currentLibrary.uuid : libraries[0]?.uuid;

						if (libraryId === undefined)
							return redirect('/onboarding', { replace: true });

						return redirect(`/${libraryId}`, { replace: true });
					}
				},
				{
					path: 'onboarding',
					lazy: () => import('./onboarding/Layout'),
					children: onboardingRoutes
				},
				{
					path: 'remote/:node/:libraryId',
					Component: (props) => <RemoteLayout cache={cache} {...props} />,
					children: [
						{
							path: 'browse',
							Component: BrowsePage
						},
						{
							// lazy: () => import('./$libraryId/Layout'),
							loader: async ({ params: { libraryId } }) => {
								// const libraries = await getCachedLibraries(cache, nonLibraryClient);

								// console.log('LOADER', libraries); // TODO

								// const library = libraries.find((l) => l.uuid === libraryId);

								// console.log(libraries, library); // TODO

								// if (!library) {
								// 	const firstLibrary = libraries[0];

								// 	if (firstLibrary)
								// 		return redirect(`/${firstLibrary.uuid}`, { replace: true });
								// 	else return redirect('/onboarding', { replace: true });
								// }

								return null;
							},
							children: [
								{
									path: '*',
									Component: () => <h1>TODO</h1>
								}
							]
							// libraryRoutes(platform)
						}
					]
				},
				{
					path: ':libraryId',
					lazy: () => import('./$libraryId/Layout'),
					loader: async ({ params: { libraryId } }) => {
						const libraries = await getCachedLibraries(cache, nonLibraryClient);
						const library = libraries.find((l) => l.uuid === libraryId);

						if (!library) {
							const firstLibrary = libraries[0];

							if (firstLibrary)
								return redirect(`/${firstLibrary.uuid}`, { replace: true });
							else return redirect('/onboarding', { replace: true });
						}

						return null;
					},
					children: libraryRoutes(platform)
				}
			]
		}
	] satisfies RouteObject[];

const ParamsSchema = z.object({ node: z.string(), libraryId: z.string() });

function RemoteLayout({ cache }: { cache: NormalisedCache }) {
	const platform = usePlatform();
	const params = useZodRouteParams(ParamsSchema);

	const [rspcClient, setRspcClient] = useState<AlphaClient<Procedures>>();
	useEffect(() => {
		const endpoint = platform.getRemoteRspcEndpoint(params.node);
		const client = initRspc<Procedures>({
			links: [
				wsBatchLink({
					url: endpoint.url
				})
			]
		});
		setRspcClient(client);

		// TODO: use this for the context
		getCachedLibraries(cache, client).then((data) => {
			console.log('todo', data);
		});

		return () => {
			// TODO: We *really* need to cleanup `client` so we aren't leaking all the resources.
		};
	}, [params.node, platform]);

	// TODO: Library context injected
	// TODO: Lazy layout

	const ctx = useRspcContext();
	return (
		<>
			{/* TODO: Maybe library context too? */}
			{rspcClient && (
				<context.Provider
					value={{
						// @ts-expect-error
						client: rspcClient,
						queryClient: ctx.queryClient
					}}
				>
					<Outlet />
				</context.Provider>
			)}
		</>
	);
}

function BrowsePage() {
	// const libraries = useLibraryContext();

	// console.log(libraries); // TODO

	return <h1>Browse</h1>;
}

/**
 * Combines the `path` segments of the current route into a single string.
 * This is useful for things like analytics, where we want the route path
 * but not the values used in the route params.
 */
const useRawRoutePath = () => {
	const { routes } = useRoutingContext();
	// `useMatches` returns a list of each matched RouteObject,
	// we grab the last one as it contains all previous route segments.
	const lastMatchId = useMatches().slice(-1)[0]?.id;

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
					['' as string, { children: routes }] as const
				) ?? [];

		return rawPath ?? '/';
	}, [lastMatchId, routes]);

	return rawPath;
};
