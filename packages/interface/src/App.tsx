import '@fontsource/inter/variable.css';
import { queryClient } from '@sd/client';
import { QueryClientProvider, defaultContext } from '@tanstack/react-query';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import { ErrorBoundary } from 'react-error-boundary';
import { MemoryRouter } from 'react-router-dom';

import { AppRouter } from './AppRouter';
import { ErrorFallback } from './ErrorFallback';
import './style.scss';

export default function SpacedriveInterface() {
	return (
		<ErrorBoundary FallbackComponent={ErrorFallback}>
			<QueryClientProvider client={queryClient} contextSharing={true}>
				{/* The `context={defaultContext}` part is required for this to work on Windows. Why, idk, don't question it */}
				{import.meta.env.MODE === 'development' && (
					<ReactQueryDevtools position="bottom-right" context={defaultContext} />
				)}
				<MemoryRouter>
					<AppRouter />
				</MemoryRouter>
			</QueryClientProvider>
		</ErrorBoundary>
	);
}
