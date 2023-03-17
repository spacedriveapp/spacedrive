import { useQueryClient } from '@tanstack/react-query';
import React, { useEffect, useRef, useState } from 'react';
import { Text } from 'react-native';
import {
	HASHING_ALGOS,
	resetOnboardingStore,
	telemetryStore,
	useBridgeMutation,
	useDebugState,
	useOnboardingStore
} from '@sd/client';
import { PulseAnimation } from '~/components/animation/lottie';
import { tw } from '~/lib/tailwind';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';
import { currentLibraryStore } from '~/utils/nav';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './GetStarted';

const CreatingLibraryScreen = ({ navigation }: OnboardingStackScreenProps<'CreatingLibrary'>) => {
	const [status, setStatus] = useState('Creating your library...');

	const queryClient = useQueryClient();

	const debugState = useDebugState();
	const obStore = useOnboardingStore();

	const createLibrary = useBridgeMutation('library.create', {
		onSuccess: (lib) => {
			resetOnboardingStore();
			queryClient.setQueryData(['library.list'], (libraries: any) => [...(libraries || []), lib]);
			// Switch to the new library
			currentLibraryStore.id = lib.uuid;
		},
		onError: () => {
			// TODO: Show toast
			resetOnboardingStore();
			navigation.navigate('GetStarted');
		}
	});

	const created = useRef(false);

	const create = async () => {
		telemetryStore.shareTelemetry = obStore.shareTelemetry;

		createLibrary.mutate({
			name: obStore.newLibraryName,
			auth: {
				type: 'TokenizedPassword',
				value: obStore.passwordSetToken || ''
			},
			algorithm: obStore.algorithm,
			hashing_algorithm: HASHING_ALGOS[obStore.hashingAlgorithm]
		});

		return;
	};

	useEffect(() => {
		if (created.current == true) return;
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
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	return (
		<OnboardingContainer>
			<Text style={tw`mb-4 text-5xl`}>🛠</Text>
			<OnboardingTitle>Creating your library</OnboardingTitle>
			<OnboardingDescription style={tw`mt-4`}>{status}</OnboardingDescription>
			<PulseAnimation style={tw`mt-2 h-10`} speed={0.3} />
		</OnboardingContainer>
	);
};

export default CreatingLibraryScreen;
