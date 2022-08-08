import '@fontsource/inter/variable.css';
import * as ToastPrimitives from '@radix-ui/react-toast';
import {
	AppProps,
	AppPropsContext,
	PairingRequest,
	queryClient,
	useBridgeMutation,
	useBridgeQuery,
	useInvalidateQuery,
	useToastNotificationsStore
} from '@sd/client';
import { QueryClientProvider } from '@tanstack/react-query';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import React, { useEffect, useState } from 'react';
import { ErrorBoundary } from 'react-error-boundary';
import { MemoryRouter } from 'react-router-dom';

import { usePairingCompleteStore } from '@sd/client/src/stores/usePairingCompleteStore';

import { Input } from '../../ui/src/Input';
import { AppRouter } from './AppRouter';
import { ErrorFallback } from './ErrorFallback';
import Dialog from './components/layout/Dialog';
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
	const [pairingRequest, setPairingRequest] = useState<PairingRequest | null>(null);
	const { toasts } = useToastNotificationsStore();

	useInvalidateQuery();

	return (
		<ErrorBoundary FallbackComponent={ErrorFallback} onReset={() => {}}>
			<QueryClientProvider client={queryClient}>
				{import.meta.env.MODE === 'development' && <ReactQueryDevtools position="bottom-right" />}
				<RouterContainer props={props} />
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
								} else if (toast.payload.type !== 'noaction') {
									console.error(
										`Found toast with unknown type '${
											(toast.payload as { type?: string }).type ?? ''
										}'`
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

	const { mutate: completeNodePairing } = useBridgeMutation('p2p.acceptPairingRequest');

	return (
		<Dialog
			open={pairingRequest !== null}
			title="Pairing Device"
			description={`Pairing with '${pairingRequest?.name ?? ''}'.`}
			ctaAction={() => {
				completeNodePairing({
					// eslint-disable-next-line @typescript-eslint/no-non-null-assertion
					peer_id: pairingRequest!.id,
					preshared_key: pairingRequestPresharedKey
				});

				// eslint-disable-next-line @typescript-eslint/no-non-null-assertion
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
