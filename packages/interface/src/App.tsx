import '@fontsource/inter/variable.css';
import { LibraryContextProvider, queryClient } from '@sd/client';
import { QueryClientProvider, defaultContext } from '@tanstack/react-query';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import dayjs from 'dayjs';
import advancedFormat from 'dayjs/plugin/advancedFormat';
import duration from 'dayjs/plugin/duration';
import relativeTime from 'dayjs/plugin/relativeTime';
import { ErrorBoundary } from 'react-error-boundary';
import { MemoryRouter, useNavigate } from 'react-router-dom';

import { AppRouter } from './AppRouter';
import { ErrorFallback } from './ErrorFallback';
import './style.scss';

dayjs.extend(advancedFormat);
dayjs.extend(relativeTime);
dayjs.extend(duration);

export default function SpacedriveInterface() {
	return (
		<ErrorBoundary FallbackComponent={ErrorFallback}>
			<QueryClientProvider client={queryClient} contextSharing={true}>
				{/* The `context={defaultContext}` part is required for this to work on Windows. Why, idk, don't question it */}
				{import.meta.env.DEV && (
					<ReactQueryDevtools
						position="bottom-right"
						context={defaultContext}
						toggleButtonProps={{
							className: 'opacity-0'
						}}
					/>
				)}
				<MemoryRouter>
					<AppRouterWrapper />
				</MemoryRouter>
			</QueryClientProvider>
		</ErrorBoundary>
	);
}

// This can't go in `<SpacedriveInterface />` cause it needs the router context but it can't go in `<AppRouter />` because that requires this context
function AppRouterWrapper() {
	const navigate = useNavigate();

	return (
		<LibraryContextProvider onNoLibrary={() => navigate('/onboarding')}>
			<AppRouter />
		</LibraryContextProvider>
	);
}
