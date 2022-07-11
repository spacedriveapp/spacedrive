import '@fontsource/inter/variable.css';
import * as ToastPrimitives from '@radix-ui/react-toast';
import { BaseTransport, ClientProvider, setTransport } from '@sd/client';
import { useCoreEvents } from '@sd/client';
import { AppProps, AppPropsContext } from '@sd/client';
import React from 'react';
import { ErrorBoundary } from 'react-error-boundary';
import { QueryClient, QueryClientProvider } from 'react-query';
import { MemoryRouter } from 'react-router-dom';
import create from 'zustand';

import { AppRouter } from './AppRouter';
import { ErrorFallback } from './ErrorFallback';
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

export interface Toast {
	title: string;
	subtitle: string;
}

export const useToastNotificationsStore = create<{
	toasts: Toast[];
	addToast: (toast: Toast) => void;
}>((set) => ({
	toasts: [],
	addToast: (toast: Toast) => set((state) => ({ toasts: [toast, ...state.toasts] }))
}));

export default function App(props: AppProps) {
	// TODO: This is a hack and a better solution should probably be found.
	// This exists so that the queryClient can be accessed within the subpackage '@sd/client'.
	// Refer to <ClientProvider /> for where this is used.
	window.ReactQueryClient ??= queryClient;

	setTransport(props.transport);

	const toasts = useToastNotificationsStore((state) => state.toasts);

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
			<ToastPrimitives.Provider>
				{/* TODO: Style this component */}
				{/* TODO: Remove the toast from the store when it is closed */}

				{toasts.map((toast) => (
					<ToastPrimitives.Root duration={3000} key={toast.title}>
						<ToastPrimitives.Title className="text-white">{toast.title}</ToastPrimitives.Title>
						<ToastPrimitives.Description className="text-white">
							{toast.subtitle}
						</ToastPrimitives.Description>
					</ToastPrimitives.Root>
				))}

				<ToastPrimitives.Viewport className="absolute p-5 rounded-md top-5 right-5 bg-red-500" />
			</ToastPrimitives.Provider>
		</>
	);
}
