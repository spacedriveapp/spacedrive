import { initRspc, wsBatchLink, type AlphaClient } from '@oscartbeaumont-sd/rspc-client/v2';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import {
	Link,
	Navigate,
	Outlet,
	redirect,
	useMatches,
	useNavigate,
	type RouteObject
} from 'react-router-dom';
import {
	CacheProvider,
	ClientContextProvider,
	context,
	context2,
	createCache,
	currentLibraryCache,
	getCachedLibraries,
	LibraryContextProvider,
	nonLibraryClient,
	NormalisedCache,
	Procedures,
	useBridgeQuery,
	useCache,
	useCachedLibraries,
	useFeatureFlag,
	useNodes,
	useRspcContext,
	WithSolid
} from '@sd/client';
import { Button, Dialogs, Toaster, z } from '@sd/ui';
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

const LibraryIdParamsSchema = z.object({ libraryId: z.string() });

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
					path: 'remote/:node',
					Component: (props) => <RemoteLayout {...props} />,
					children: [
						{
							path: 'browse',
							Component: BrowsePage
						},
						{
							path: ':libraryId',
							Component: () => {
								const params = useZodRouteParams(LibraryIdParamsSchema);
								const result = useBridgeQuery(['library.list']);
								useNodes(result.data?.nodes);
								const libraries = useCache(result.data?.items);

								const library = libraries?.find((l) => l.uuid === params.libraryId);

								useEffect(() => {
									if (!result.data) return;

									if (!library) {
										alert('Library not found');
										// TODO: Redirect
									}
								});

								if (!library) return <></>; // TODO: Using suspense for loading

								return (
									<ClientContextProvider currentLibraryId={params.libraryId}>
										<LibraryContextProvider library={library}>
											<div className="w-full bg-orange-500 text-center text-white">
												YOUR ON A REMOTE NODE
											</div>
											<Outlet />
										</LibraryContextProvider>
									</ClientContextProvider>
								);
							},
							children: [
								{
									path: '*',
									lazy: () => import('./$libraryId/Layout'),
									children: libraryRoutes(platform)
								}
							]
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

const ParamsSchema = z.object({ node: z.string() });

function RemoteLayout() {
	const platform = usePlatform();
	const params = useZodRouteParams(ParamsSchema);

	// TODO: The caches should instead be prefixed by the remote node ID, instead of completely being recreated but that's too hard to do right now.
	const [rspcClient, setRspcClient] =
		useState<
			[AlphaClient<Procedures>, AlphaClient<Procedures>, QueryClient, NormalisedCache]
		>();
	useEffect(() => {
		const endpoint = platform.getRemoteRspcEndpoint(params.node);

		const links = [
			wsBatchLink({
				url: endpoint.url
			})
		];

		const client = initRspc<Procedures>({
			links
		});
		const libraryClient = initRspc<Procedures>({
			links
		});
		const cache = createCache();
		setRspcClient([client, libraryClient, new QueryClient(), cache]);

		return () => {
			// TODO: We *really* need to cleanup `client` so we aren't leaking all the resources.
		};
	}, [params.node, platform]);

	// TODO: Library context injected
	// TODO: Lazy layout

	return (
		<>
			{/* TODO: Maybe library context too? */}
			{rspcClient && (
				<QueryClientProvider client={rspcClient[2]}>
					<CacheProvider cache={rspcClient[3]}>
						<context.Provider
							value={{
								// @ts-expect-error
								client: rspcClient[0],
								queryClient: rspcClient[2]
							}}
						>
							<context2.Provider
								value={{
									// @ts-expect-error
									client: rspcClient[1],
									queryClient: rspcClient[2]
								}}
							>
								<Outlet />
							</context2.Provider>
						</context.Provider>
					</CacheProvider>
				</QueryClientProvider>
			)}
		</>
	);
}

function BrowsePage() {
	const navigate = useNavigate();
	const result = useBridgeQuery(['library.list']);
	useNodes(result.data?.nodes);
	const libraries = useCache(result.data?.items);

	return (
		<div className="flex flex-col">
			<h1>Browse Libraries On Remote Node:</h1>
			{libraries?.map((l) => (
				<Button
					key={l.uuid}
					variant="accent"
					// TODO: Take into account Windows vs Mac vs Linux with the default `path`
					onClick={() => navigate(`../${l.uuid}/ephemeral/0-0?path=/System/Volumes/Data`)}
				>
					{l.config.name}
				</Button>
			))}
		</div>
	);
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
