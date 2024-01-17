import dayjs from 'dayjs';
import advancedFormat from 'dayjs/plugin/advancedFormat';
import duration from 'dayjs/plugin/duration';
import relativeTime from 'dayjs/plugin/relativeTime';
import { PropsWithChildren, Suspense } from 'react';
import { RouterProvider, RouterProviderProps } from 'react-router-dom';
import {
	InteropProviderReact,
	P2PContextProvider,
	useBridgeSubscription,
	useInvalidateQuery,
	useLoadBackendFeatureFlags
} from '@sd/client';
import { toast, TooltipProvider } from '@sd/ui';

import { createRoutes } from './app';
import { SpacedropProvider } from './app/$libraryId/Spacedrop';
import { P2P, useP2PErrorToast } from './app/p2p';
import { Devtools } from './components/Devtools';
import { WithPrismTheme } from './components/TextViewer/prism';
import ErrorFallback, { BetterErrorBoundary } from './ErrorFallback';
import { useTheme } from './hooks';
import { RouterContext, RoutingContext } from './RoutingContext';

export * from './app';
export { ErrorPage } from './ErrorFallback';
export * from './TabsContext';
export * from './util/keybind';
export * from './util/Platform';

dayjs.extend(advancedFormat);
dayjs.extend(relativeTime);
dayjs.extend(duration);

import('@sentry/browser').then(({ init, Integrations }) => {
	init({
		dsn: 'https://2fb2450aabb9401b92f379b111402dbc@o1261130.ingest.sentry.io/4504053670412288',
		environment: import.meta.env.MODE,
		defaultIntegrations: false,
		integrations: [new Integrations.HttpContext(), new Integrations.Dedupe()]
	});
});

export type Router = RouterProviderProps['router'];

export function SpacedriveRouterProvider(props: {
	routing: {
		routes: ReturnType<typeof createRoutes>;
		visible: boolean;
		router: Router;
		currentIndex: number;
		maxIndex: number;
	};
}) {
	return (
		<RouterContext.Provider value={props.routing.router}>
			<RoutingContext.Provider
				value={{
					routes: props.routing.routes,
					visible: props.routing.visible,
					currentIndex: props.routing.currentIndex,
					maxIndex: props.routing.maxIndex
				}}
			>
				<RouterProvider
					router={props.routing.router}
					future={{
						v7_startTransition: true
					}}
				/>
			</RoutingContext.Provider>
		</RouterContext.Provider>
	);
}

export function SpacedriveInterfaceRoot({ children }: PropsWithChildren) {
	useLoadBackendFeatureFlags();
	useP2PErrorToast();
	useInvalidateQuery();
	useTheme();

	useBridgeSubscription(['notifications.listen'], {
		onData({ data: { title, content, kind }, expires }) {
			toast({ title, body: content }, { type: kind });
		}
	});

	return (
		<Suspense>
			<BetterErrorBoundary FallbackComponent={ErrorFallback}>
				<InteropProviderReact>
					<TooltipProvider>
						<P2PContextProvider>
							<P2P />
							<Devtools />
							<WithPrismTheme />
							<SpacedropProvider />
							{children}
						</P2PContextProvider>
					</TooltipProvider>
				</InteropProviderReact>
			</BetterErrorBoundary>
		</Suspense>
	);
}
