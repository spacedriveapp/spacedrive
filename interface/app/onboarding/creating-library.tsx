/* eslint-disable react-hooks/exhaustive-deps */
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import {
	Algorithm,
	HASHING_ALGOS,
	resetOnboardingStore,
	useBridgeMutation,
	useDebugState,
	useOnboardingStore
} from '@sd/client';
import { Loader } from '@sd/ui';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './Layout';
import { useUnlockOnboardingScreen } from './Progress';

export default function OnboardingCreatingLibrary() {
	const navigate = useNavigate();
	const queryClient = useQueryClient();

	const [status, setStatus] = useState('Creating your library...');

	useUnlockOnboardingScreen();

	const debugState = useDebugState();

	const createLibrary = useBridgeMutation('library.create', {
		onSuccess: (library) => {
			queryClient.setQueryData(['library.list'], (libraries: any) => [
				...(libraries || []),
				library
			]);
			resetOnboardingStore();
			navigate(`/${library.uuid}/overview`);
		},
		onError: () => {
			resetOnboardingStore();
			navigate('/onboarding/');
		}
	});

	const ob_store = useOnboardingStore();

	const create = async () => {
		createLibrary.mutate({
			name: ob_store.newLibraryName,
			auth: {
				type: 'TokenizedPassword',
				value: ob_store.passwordSetToken || ''
			},
			algorithm: ob_store.algorithm as Algorithm,
			hashing_algorithm: HASHING_ALGOS[ob_store.hashingAlgorithm]
		});

		return;
	};

	useEffect(() => {
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
