import { Integrations, init } from '@sentry/browser';
import { QueryClientProvider, defaultContext } from '@tanstack/react-query';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import dayjs from 'dayjs';
import advancedFormat from 'dayjs/plugin/advancedFormat';
import duration from 'dayjs/plugin/duration';
import relativeTime from 'dayjs/plugin/relativeTime';
import { ErrorBoundary } from 'react-error-boundary';
import { MemoryRouter } from 'react-router-dom';
import { queryClient, useDebugState } from '@sd/client';
import { Dialogs } from '@sd/ui';
import App from './src';
import { ErrorFallback } from './src/ErrorFallback';

export * from './src/util/keybind';
export * from './src/util/Platform';

dayjs.extend(advancedFormat);
dayjs.extend(relativeTime);
dayjs.extend(duration);

init({
	dsn: 'https://2fb2450aabb9401b92f379b111402dbc@o1261130.ingest.sentry.io/4504053670412288',
	environment: import.meta.env.MODE,
	defaultIntegrations: false,
	integrations: [new Integrations.HttpContext(), new Integrations.Dedupe()]
});

const Devtools = () => {
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
};

export const SpacedriveInterface = () => (
	<ErrorBoundary FallbackComponent={ErrorFallback}>
		<QueryClientProvider client={queryClient} contextSharing={true}>
			<Devtools />
			<MemoryRouter>
				<App />
			</MemoryRouter>
			<Dialogs />
		</QueryClientProvider>
	</ErrorBoundary>
);
