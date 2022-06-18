import '@fontsource/inter/variable.css';
import { BaseTransport, ClientProvider, setTransport } from '@sd/client';
import React from 'react';
import { ErrorBoundary } from 'react-error-boundary';
import { QueryClient, QueryClientProvider } from 'react-query';
import { MemoryRouter } from 'react-router-dom';

import { AppRouter } from './AppRouter';
import { ErrorFallback } from './ErrorFallback';
import { useCoreEvents } from './hooks/useCoreEvents';
import './style.scss';

const queryClient = new QueryClient();

export const AppPropsContext = React.createContext<AppProps | null>(null);

export type Platform = 'browser' | 'macOS' | 'windows' | 'linux';

export interface AppProps {
	transport: BaseTransport;
	platform: Platform;
	convertFileSrc: (url: string) => string;
	openDialog: (options: { directory?: boolean }) => Promise<string | string[] | null>;
	onClose?: () => void;
	onMinimize?: () => void;
	onFullscreen?: () => void;
	onOpen?: (path: string) => void;
	isFocused?: boolean;
	demoMode?: boolean;
}

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
