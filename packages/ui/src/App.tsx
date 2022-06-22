import '@fontsource/inter/variable.css';
import { BaseTransport, ClientProvider, setTransport } from '@sd/client';
import React from 'react';
import { ErrorBoundary } from 'react-error-boundary';
import { QueryClient, QueryClientProvider } from 'react-query';
import { MemoryRouter } from 'react-router-dom';

import { AppProps, AppPropsContext } from './AppPropsContext';
import { AppRouter } from './AppRouter';
import { ErrorFallback } from './ErrorFallback';
import { useCoreEvents } from './hooks/useCoreEvents';
import './style.scss';

const queryClient = new QueryClient();

function RouterContainer() {
	useCoreEvents();
	return (
		<MemoryRouter>
			<AppRouter />
		</MemoryRouter>
	);
}

export default function App(props: AppProps) {
	// TODO: This is a hack and a better solution should probably be found.
	// This exists so that the queryClient can be accessed within the subpackage '@sd/client'.
	// Refer to <ClientProvider /> for where this is used.
	window.ReactQueryClient ??= queryClient;

	setTransport(props.transport);

	return (
		<>
			<ErrorBoundary FallbackComponent={ErrorFallback} onReset={() => {}}>
				<QueryClientProvider client={queryClient} contextSharing={false}>
					<AppPropsContext.Provider value={Object.assign({ isFocused: true }, props)}>
						<ClientProvider>
							<RouterContainer />
						</ClientProvider>
					</AppPropsContext.Provider>
				</QueryClientProvider>
			</ErrorBoundary>
		</>
	);
}
