/* eslint-disable react-hooks/exhaustive-deps */
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router';
import {
	Algorithm,
	HASHING_ALGOS,
	resetOnboardingStore,
	telemetryStore,
	useBridgeMutation,
	useDebugState,
	useOnboardingStore,
	usePlausibleEvent
} from '@sd/client';
import { Loader } from '@sd/ui';
import { usePlatform } from '~/util/Platform';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './Layout';
import { useUnlockOnboardingScreen } from './Progress';

export default function OnboardingCreatingLibrary() {
	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const debugState = useDebugState();
	const platform = usePlatform();
	const submitPlausibleEvent = usePlausibleEvent({ platformType: platform.platform });

	const [status, setStatus] = useState('Creating your library...');

	useUnlockOnboardingScreen();

	const createLibrary = useBridgeMutation('library.create', {
		onSuccess: (library) => {
			queryClient.setQueryData(['library.list'], (libraries: any) => [
				...(libraries || []),
				library
			]);

			submitPlausibleEvent({ event: { type: 'libraryCreate' } });

			resetOnboardingStore();
			navigate(`/${library.uuid}/overview`);
		},
		onError: () => {
			resetOnboardingStore();
			navigate('/onboarding/');
		}
	});

	const obStore = useOnboardingStore();

	const create = async () => {
		// opted to place this here as users could change their mind before library creation/onboarding finalization
		// it feels more fitting to configure it here (once)
		telemetryStore.shareTelemetry = obStore.shareTelemetry;

		createLibrary.mutate({
			name: obStore.newLibraryName,
			auth: {
				type: 'TokenizedPassword',
				value: obStore.passwordSetToken || ''
			},
			algorithm: obStore.algorithm as Algorithm,
			hashing_algorithm: HASHING_ALGOS[obStore.hashingAlgorithm]
		});

		return;
	};

	const created = useRef(false);

	useEffect(() => {
		if (created.current) return;
		created.current = true;
		create();
		const timer = setTimeout(() => {
			setStatus('Almost done...');
		}, 2000);
		const timer2 = setTimeout(() => {
			if (debugState.enabled) {
				setStatus(`You're running in development, this will take longer...`);
			}
		}, 5000);
		return () => {
			clearTimeout(timer);
			clearTimeout(timer2);
		};
	}, []);

	return (
		<OnboardingContainer>
			<span className="text-6xl">🛠</span>
			<OnboardingTitle>Creating your library</OnboardingTitle>
			<OnboardingDescription>{status}</OnboardingDescription>
			<Loader className="mt-5" />
		</OnboardingContainer>
	);
}
