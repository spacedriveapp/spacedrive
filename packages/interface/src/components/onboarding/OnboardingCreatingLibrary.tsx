import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import {
	Algorithm,
	resetOnboardingStore,
	useBridgeMutation,
	useDebugState,
	useOnboardingStore
} from '@sd/client';
import { Button, Card, Loader } from '@sd/ui';
import { getHashingAlgorithmSettings } from '../../screens/settings/library/KeysSetting';
import { useUnlockOnboardingScreen } from './OnboardingProgress';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './OnboardingRoot';

export default function OnboardingCreatingLibrary() {
	const navigate = useNavigate();
	const queryClient = useQueryClient();

	const [status, setStatus] = useState('Creating your library...');
	const [error, setError] = useState<string | null>(null);

	useUnlockOnboardingScreen();

	const debugState = useDebugState();

	const createLibrary = useBridgeMutation('library.create', {
		onSuccess: (library) => {
			queryClient.setQueryData(['library.list'], (libraries: any) => [
				...(libraries || []),
				library
			]);
			resetOnboardingStore();
			navigate('/overview/');
		},
		onError: (err: any) => {
			console.log({ libraryCreationError: err });
			setError(err.message);
			alert('fuck');
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
			hashing_algorithm: getHashingAlgorithmSettings(ob_store.hashingAlgorithm)
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
			{error && (
				<div className="flex w-[450px] mt-4 flex-col">
					<Card className="flex flex-col mt-2 bg-red-500/20 border-red-500/10">
						<span className="text-sm font-medium text-red-500">{error}</span>
					</Card>
					<div className="flex items-center justify-between w-full mt-7">
						<Button
							onClick={() => {
								resetOnboardingStore();
								navigate('/onboarding/');
							}}
							variant="outline"
							size="sm"
						>
							Start again
						</Button>
					</div>
				</div>
			)}
		</OnboardingContainer>
	);
}
