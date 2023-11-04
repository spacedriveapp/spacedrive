import { init, Integrations } from '@sentry/browser';

import '@fontsource/inter/variable.css';

import dayjs from 'dayjs';
import advancedFormat from 'dayjs/plugin/advancedFormat';
import duration from 'dayjs/plugin/duration';
import relativeTime from 'dayjs/plugin/relativeTime';
import { RouterProvider, RouterProviderProps } from 'react-router-dom';
import {
	CacheProvider,
	NotificationContextProvider,
	P2PContextProvider,
	useLoadBackendFeatureFlags
} from '@sd/client';
import { TooltipProvider } from '@sd/ui';

import { P2P } from './app/p2p';
import { Devtools } from './components/Devtools';
import { WithPrismTheme } from './components/TextViewer/prism';
import ErrorFallback, { BetterErrorBoundary } from './ErrorFallback';

export { ErrorPage } from './ErrorFallback';
export * from './app';
export * from './util/Platform';
export * from './util/keybind';

dayjs.extend(advancedFormat);
dayjs.extend(relativeTime);
dayjs.extend(duration);

init({
	dsn: 'https://2fb2450aabb9401b92f379b111402dbc@o1261130.ingest.sentry.io/4504053670412288',
	environment: import.meta.env.MODE,
	defaultIntegrations: false,
	integrations: [new Integrations.HttpContext(), new Integrations.Dedupe()]
});

export const SpacedriveInterface = (props: { router: RouterProviderProps['router'] }) => {
	useLoadBackendFeatureFlags();

	return (
		<BetterErrorBoundary FallbackComponent={ErrorFallback}>
			<CacheProvider>
				<TooltipProvider>
					<P2PContextProvider>
						<NotificationContextProvider>
							<P2P />
							<Devtools />
							<WithPrismTheme />
							<RouterProvider router={props.router} />
						</NotificationContextProvider>
					</P2PContextProvider>
				</TooltipProvider>
			</CacheProvider>
		</BetterErrorBoundary>
	);
};
