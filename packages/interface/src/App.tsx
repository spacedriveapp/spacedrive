import '@fontsource/inter/variable.css';
import {
	AppProps,
	AppPropsContext,
	queryClient,
	useBridgeQuery,
	useInvalidateQuery
} from '@sd/client';
import { QueryClientProvider, defaultContext } from '@tanstack/react-query';
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
		setAppProps((appProps) => ({
			...appProps,
			data_path: client?.data_path
		}));
	}, [client?.data_path]);

	return (
		<AppPropsContext.Provider value={Object.assign({ isFocused: true }, appProps)}>
			<MemoryRouter>
				<AppRouter />
			</MemoryRouter>
		</AppPropsContext.Provider>
	);
}

export default function SpacedriveInterface(props: AppProps) {
	useInvalidateQuery();

	// hotfix for bug where props are not updated, not sure of the cause
	if (props.platform === 'unknown') {
		// this should be a loading screen if we can't fix the issue above
		return <></>;
	}

	return (
		<ErrorBoundary FallbackComponent={ErrorFallback}>
			<QueryClientProvider client={queryClient} contextSharing={true}>
				{/* The `context={defaultContext}` part is required for this to work on Windows. Why, idk, don't question it */}
				{import.meta.env.MODE === 'development' && (
					<ReactQueryDevtools position="bottom-right" context={defaultContext} />
				)}
				<RouterContainer props={props} />
			</QueryClientProvider>
		</ErrorBoundary>
	);
}
