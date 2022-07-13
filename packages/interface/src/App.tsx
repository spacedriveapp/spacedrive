import '@fontsource/inter/variable.css';
import * as ToastPrimitives from '@radix-ui/react-toast';
import {
	BaseTransport,
	ClientProvider,
	PairingRequest,
	setTransport,
	useBridgeCommand,
	useToastNotificationsStore
} from '@sd/client';
import { useCoreEvents } from '@sd/client';
import { AppProps, AppPropsContext } from '@sd/client';
import React, { useEffect, useState } from 'react';
import { ErrorBoundary } from 'react-error-boundary';
import { QueryClient, QueryClientProvider } from 'react-query';
import { MemoryRouter } from 'react-router-dom';
import create from 'zustand';

import { usePairingCompleteStore } from '@sd/client/src/stores/usePairingCompleteStore';

import { Input } from '../../ui/src/Input';
import { AppRouter } from './AppRouter';
import { ErrorFallback } from './ErrorFallback';
import Dialog from './components/layout/Dialog';
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

	const [pairingRequest, setPairingRequest] = useState<PairingRequest | null>(null);
	const { toasts, addToast } = useToastNotificationsStore();

	return (
		<ErrorBoundary FallbackComponent={ErrorFallback} onReset={() => {}}>
			<QueryClientProvider client={queryClient} contextSharing={false}>
				<AppPropsContext.Provider value={Object.assign({ isFocused: true }, props)}>
					<ClientProvider>
						<RouterContainer />
						<ToastPrimitives.Provider>
							{/* TODO: Style this component */}
							{/* Ability to close toast manually with button */}
							{/* TODO: Remove the toast from the store when it is closed */}
							{/* Animate toast on and off screen */}

							{toasts.map((toast) => (
								<ToastPrimitives.Root
									duration={3000}
									key={toast.title}
									className="bg-red-500 rounded-md p-2"
									onClick={() => {
										if (toast.payload.type === 'pairingRequest') {
											setPairingRequest(toast.payload.data);
										} else if (toast.payload.type === 'noaction') {
										} else {
											console.error(
												`Found toast with unknown type '${(toast.payload as any).type || ''}'`
											);
										}
									}}
								>
									<ToastPrimitives.Title className="text-white text-lg">
										{toast.title}
									</ToastPrimitives.Title>
									<ToastPrimitives.Description className="text-white text-sm">
										{toast.subtitle}
									</ToastPrimitives.Description>
								</ToastPrimitives.Root>
							))}

							<ToastPrimitives.Viewport className="absolute p-5 top-5 right-5 flex-col space-y-4" />
						</ToastPrimitives.Provider>
						<PairingCompleteDialog
							pairingRequest={pairingRequest}
							setPairingRequest={setPairingRequest}
						/>
					</ClientProvider>
				</AppPropsContext.Provider>
			</QueryClientProvider>
		</ErrorBoundary>
	);
}

function PairingCompleteDialog({
	pairingRequest,
	setPairingRequest
}: {
	pairingRequest: PairingRequest | null;
	setPairingRequest: (v: PairingRequest | null) => void;
}) {
	const [pairingRequestPresharedKey, setPairingRequestPresharedKey] = useState('');
	const { pairingRequestCallbacks } = usePairingCompleteStore();

	const { mutate: completeNodePairing } = useBridgeCommand('AcceptPairingRequest');

	return (
		<Dialog
			open={pairingRequest !== null}
			title="Pairing Device"
			description={`Pairing with '${pairingRequest?.name || ''}'.`}
			ctaAction={() => {
				completeNodePairing({
					peer_id: pairingRequest?.id,
					preshared_key: pairingRequestPresharedKey
				});

				pairingRequestCallbacks.set(pairingRequest!.id, () => {
					setPairingRequest(null);
				});
			}}
			ctaClose={() => {
				setPairingRequest(null);
			}}
			ctaLabel="Pair"
			trigger={<></>}
		>
			<span className="mb-1 text-xs font-bold uppercase text-gray-450">
				Password shown on the remote device.
			</span>
			<Input
				value={pairingRequestPresharedKey}
				onChange={(e) => setPairingRequestPresharedKey(e.currentTarget.value)}
				className="w-full mt-2"
			/>
		</Dialog>
	);
}
