import '@fontsource/inter/variable.css';
import { BaseTransport, ClientProvider, setTransport, useBridgeQuery } from '@sd/client';
import { useCoreEvents } from '@sd/client';
import { AppProps, AppPropsContext } from '@sd/client';
import React, { useEffect, useState } from 'react';
import { ErrorBoundary } from 'react-error-boundary';
import { QueryClient, QueryClientProvider } from 'react-query';
import { MemoryRouter } from 'react-router-dom';

import { AppRouter } from './AppRouter';
import { ErrorFallback } from './ErrorFallback';
import './style.scss';

const queryClient = new QueryClient();

function RouterContainer(props: { props: AppProps }) {
	useCoreEvents();
	const [appProps, setAppProps] = useState(props.props);
	const { data: client } = useBridgeQuery('NodeGetState');

	useEffect(() => {
		setAppProps({
			...appProps,
			data_path: client?.data_path
		});
	}, [client?.data_path]);

	return (
		<AppPropsContext.Provider value={Object.assign({ isFocused: true }, appProps)}>
			<MemoryRouter>
				<AppRouter />
			</MemoryRouter>
		</AppPropsContext.Provider>
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
					<ClientProvider>
						<RouterContainer props={props} />
					</ClientProvider>
				</QueryClientProvider>
			</ErrorBoundary>
		</>
	);
}
