import '@fontsource/inter/variable.css';
import {
	AppProps,
	AppPropsContext,
	queryClient,
	useBridgeQuery,
	useInvalidateQuery
} from '@sd/client';
import { QueryClientProvider } from '@tanstack/react-query';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import React, { useEffect, useState } from 'react';
import { ErrorBoundary } from 'react-error-boundary';
import { MemoryRouter } from 'react-router-dom';

import { AppRouter } from './AppRouter';
import { ErrorFallback } from './ErrorFallback';
import './style.scss';

function RouterContainer(props: { props: AppProps }) {
	const [appProps, setAppProps] = useState(props.props);
	const { data: client } = useBridgeQuery(['getNode']);

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
	useInvalidateQuery();

	return (
		<ErrorBoundary FallbackComponent={ErrorFallback} onReset={() => {}}>
			<QueryClientProvider client={queryClient}>
				{import.meta.env.MODE === 'development' && <ReactQueryDevtools position="bottom-right" />}
				<RouterContainer props={props} />
			</QueryClientProvider>
		</ErrorBoundary>
	);
}
