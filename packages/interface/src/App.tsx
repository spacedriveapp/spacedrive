import {
	Dedupe as DedupeIntegration,
	HttpContext as HttpContextIntegration,
	init
} from '@sentry/browser';
import '@fontsource/inter/variable.css';
import { defaultContext, QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import dayjs from 'dayjs';
import advancedFormat from 'dayjs/plugin/advancedFormat';
import duration from 'dayjs/plugin/duration';
import relativeTime from 'dayjs/plugin/relativeTime';
import { ErrorBoundary } from 'react-error-boundary';
import { BrowserRouter, MemoryRouter } from 'react-router-dom';
import { useDebugState } from '@sd/client';
import { Dialogs } from '@sd/ui';
import { AppRouter } from './AppRouter';
import { ErrorFallback } from './ErrorFallback';
import './style.scss';

dayjs.extend(advancedFormat);
dayjs.extend(relativeTime);
dayjs.extend(duration);

init({
	dsn: 'https://2fb2450aabb9401b92f379b111402dbc@o1261130.ingest.sentry.io/4504053670412288',
	environment: import.meta.env.MODE,
	defaultIntegrations: false,
	integrations: [new HttpContextIntegration(), new DedupeIntegration()]
});

export default function SpacedriveInterface({ router }: { router: 'memory' | 'browser' }) {
	const Router = router === 'memory' ? MemoryRouter : BrowserRouter;
	return (
		<ErrorBoundary FallbackComponent={ErrorFallback}>
			<Devtools />
			<Router>
				<AppRouter />
			</Router>
			<Dialogs />
		</ErrorBoundary>
	);
}

function Devtools() {
	const debugState = useDebugState();

	// The `context={defaultContext}` part is required for this to work on Windows. Why, idk, don't question it
	return debugState.reactQueryDevtools !== 'disabled' ? (
		<ReactQueryDevtools
			position="bottom-right"
			context={defaultContext}
			toggleButtonProps={{
				className: debugState.reactQueryDevtools === 'invisible' ? 'opacity-0' : ''
			}}
		/>
	) : null;
}
