import { useState } from 'react';
import { useNavigate } from 'react-router';
import { getOnboardingStore, useBridgeMutation, useOnboardingStore } from '@sd/client';
import { Button, Card, PasswordMeter } from '@sd/ui';
import { Form, PasswordInput, useZodForm, z } from '@sd/ui/src/forms';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './Layout';
import { useUnlockOnboardingScreen } from './Progress';

const schema = z.object({
	password: z.string(),
	password_validate: z.string(),
	algorithm: z.string(),
	hashing_algorithm: z.string()
});

export default function OnboardingNewLibrary() {
	const navigate = useNavigate();
	// const queryClient = useQueryClient();

	const [showPasswordValidate, setShowPasswordValidate] = useState(false);

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

	const tokenizeSensitiveKey = useBridgeMutation('nodes.tokenizeSensitiveKey', {
		onSuccess: (data) => {
			getOnboardingStore().passwordSetToken = data.token;
			navigate('/onboarding/privacy');
		},
		onError: (err: any) => {
			alert(err);
		}
	});

	const ob_store = useOnboardingStore();

	const onSubmit = form.handleSubmit(async (data) => {
		if (data.password !== data.password_validate) {
			if (!showPasswordValidate) {
				setShowPasswordValidate(true);
				// focus on password validate
			} else {
				form.setError('password_validate', {
					type: 'manual',
					message: 'Passwords do not match'
				});
			}
		} else {
			tokenizeSensitiveKey.mutate({
				secret_key: data.password
			});
		}
	});

	return (
		<Form form={form} onSubmit={onSubmit}>
			<OnboardingContainer>
				{/* <OnboardingImg src={Database} /> */}
				<OnboardingTitle>Set a master password</OnboardingTitle>
				<OnboardingDescription>
					This will be used to encrypt your library and/or open the built-in key manager.
				</OnboardingDescription>

				<div className="mt-4 flex w-[450px] flex-col">
					{form.formState.errors.password_validate && (
						<Card className="mt-2 flex flex-col border-red-500/10 bg-red-500/20">
							<span className="text-sm font-medium text-red-500">
								{form.formState.errors.password_validate.message}
							</span>
						</Card>
					)}
					<div className="my-2 flex grow">
						<PasswordInput
							{...form.register('password')}
							size="md"
							autoFocus
							className="w-full"
							disabled={form.formState.isSubmitting}
						/>
					</div>
					{showPasswordValidate && (
						<div className="mb-2 flex grow">
							<PasswordInput
								{...form.register('password_validate')}
								size="md"
								placeholder="Confirm password"
								autoFocus
								className="w-full"
								disabled={form.formState.isSubmitting}
							/>
						</div>
					)}

					<div className="mt-3 flex flex-col">
						<PasswordMeter password={form.watch('password')} />
					</div>
					<div className="mt-7 flex w-full justify-between">
						{!ob_store.passwordSetToken ? (
							<Button
								disabled={form.formState.isSubmitting}
								type="submit"
								variant="outline"
								size="sm"
							>
								Continue without password →
							</Button>
						) : (
							<Button
								disabled={form.formState.isSubmitting}
								variant="outline"
								size="sm"
								onClick={() => {
									getOnboardingStore().passwordSetToken = null;
									form.reset();
								}}
							>
								Remove password
							</Button>
						)}
						<Button disabled={form.formState.isSubmitting} type="submit" variant="accent" size="sm">
							Set password
						</Button>
					</div>
				</div>
			</OnboardingContainer>
		</Form>
	);
}
