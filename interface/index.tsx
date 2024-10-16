import dayjs from 'dayjs';
import advancedFormat from 'dayjs/plugin/advancedFormat';
import duration from 'dayjs/plugin/duration';
import relativeTime from 'dayjs/plugin/relativeTime';
import { PropsWithChildren, Suspense } from 'react';
import { I18nextProvider } from 'react-i18next';
import { RouterProvider, RouterProviderProps } from 'react-router-dom';
import {
	InteropProviderReact,
	P2PContextProvider,
	useBridgeMutation,
	useBridgeSubscription,
	useInvalidateQuery,
	useLoadBackendFeatureFlags
} from '@sd/client';
import { dialogManager, toast, TooltipProvider } from '@sd/ui';
import RequestAddDialog from '~/components/RequestAddDialog';

import { createRoutes } from './app';
import { SpacedropProvider } from './app/$libraryId/Spacedrop';
import i18n from './app/I18n';
import { Devtools } from './components/Devtools';
import { WithPrismTheme } from './components/TextViewer/prism';
import ErrorFallback, { BetterErrorBoundary } from './ErrorFallback';
import { useTheme } from './hooks';
import { RouterContext, RoutingContext } from './RoutingContext';

export * from './app';
export { ErrorPage } from './ErrorFallback';
export * from './TabsContext';
export * from './util/events';
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
		tabId: string;
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
					tabId: props.routing.tabId,
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
	useInvalidateQuery();
	useTheme();

	useBridgeSubscription(['notifications.listen'], {
		onData({ data: { title, content, kind }, expires }) {
			toast({ title, body: content }, { type: kind });
		}
	});

	const userResponse = useBridgeMutation('cloud.userResponse');

	useBridgeSubscription(['cloud.listenCloudServicesNotifications'], {
		onData: (d) => {
			console.log('Received cloud service notification', d);
			switch (d.kind) {
				case 'ReceivedJoinSyncGroupRequest':
					// WARNING: This is a debug solution to accept the device into the sync group. THIS SHOULD NOT MAKE IT TO PRODUCTION
					userResponse.mutate({
						kind: 'AcceptDeviceInSyncGroup',
						data: {
							ticket: d.data.ticket,
							accepted: {
								id: d.data.sync_group.library.pub_id,
								name: d.data.sync_group.library.name,
								description: null
							}
						}
					});
					// TODO: Move the code above into the dialog below (@Rocky43007)
					// dialogManager.create((dp) => (
					// 	<RequestAddDialog
					// 		device_model={'MacBookPro'}
					// 		device_name={"Arnab's Macbook"}
					// 		library_name={"Arnab's Library"}
					// 		{...dp}
					// 	/>
					// ));
					break;
				default:
					toast({ title: 'Cloud Service Notification', body: d.kind }, { type: 'info' });
					break;
			}
		}
	});

	return (
		<Suspense>
			<I18nextProvider i18n={i18n}>
				<BetterErrorBoundary FallbackComponent={ErrorFallback}>
					<InteropProviderReact>
						<TooltipProvider>
							<P2PContextProvider>
								<Devtools />
								<WithPrismTheme />
								<SpacedropProvider />
								{children}
							</P2PContextProvider>
						</TooltipProvider>
					</InteropProviderReact>
				</BetterErrorBoundary>
			</I18nextProvider>
		</Suspense>
	);
}
