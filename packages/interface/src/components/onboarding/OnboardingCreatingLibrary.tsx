import { Algorithm, getOnboardingStore, useBridgeMutation, useOnboardingStore } from '@sd/client';
import { Button, Card, forms } from '@sd/ui';
import { useQueryClient } from '@tanstack/react-query';
import { Eye, EyeSlash } from 'phosphor-react';
import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router';

import { getHashingAlgorithmSettings } from '../../screens/settings/library/KeysSetting';
import { PasswordMeter } from '../key/PasswordMeter';
import { useUnlockOnboardingScreen } from './OnboardingProgress';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './OnboardingRoot';

const { PasswordShowHideInput, z, useZodForm, Form } = forms;

const schema = z.object({
	password: z.string(),
	password_validate: z.string(),
	algorithm: z.string(),
	hashing_algorithm: z.string()
});

export default function OnboardingNewLibrary() {
	const navigate = useNavigate();
	const queryClient = useQueryClient();

	const form = useZodForm({
		schema,
		defaultValues: {
			password: '',
			password_validate: '',
			algorithm: 'XChaCha20Poly1305',
			hashing_algorithm: 'Argon2id-s'
		}
	});

	useUnlockOnboardingScreen();

	const createLibrary = useBridgeMutation('library.create', {
		onSuccess: (library) => {
			queryClient.setQueryData(['library.list'], (libraries: any) => [
				...(libraries || []),
				library
			]);
			form.reset();
		},
		onError: (err: any) => {
			alert(err);
		}
	});

	const ob_store = useOnboardingStore();

	const _onSubmit = form.handleSubmit(async (data) => {
		getOnboardingStore().hasSetPassword = true;
		// actually create library
		// createLibrary.mutate({ name:ob_store.newLibraryName, ...data, secret_key: null});

		await createLibrary
			.mutateAsync({
				name: ob_store.newLibraryName,
				...data,
				algorithm: data.algorithm as Algorithm,
				hashing_algorithm: getHashingAlgorithmSettings(data.hashing_algorithm),
				secret_key: null // temp
			})
			.then(() => {
				navigate('/onboarding/privacy');
			});

		return;
	});

	const [status, setStatus] = useState('Creating your library...');

	useEffect(() => {
		const timer = setTimeout(() => {
			setStatus('Almost done...');
		}, 2000);
		return () => clearTimeout(timer);
	}, []);

	return (
		<Form form={form} onSubmit={_onSubmit}>
			<OnboardingContainer>
				<OnboardingTitle>One moment</OnboardingTitle>
				<OnboardingDescription>{status}</OnboardingDescription>

				<div className="flex w-[450px] mt-4 flex-col">
					{form.formState.errors.password_validate && (
						<Card className="flex flex-col mt-2 bg-red-500/20 border-red-500/10">
							<span className="text-sm font-medium text-red-500">
								{form.formState.errors.password_validate.message}
							</span>
						</Card>
					)}

					<div className="flex flex-col mt-3">
						<PasswordMeter password={form.watch('password')} />
					</div>
					<div className="flex justify-between w-full mt-7">
						<Button
							disabled={form.formState.isSubmitting}
							type="submit"
							variant="outline"
							size="sm"
						>
							Try again
						</Button>
					</div>
				</div>
			</OnboardingContainer>
		</Form>
	);
}
