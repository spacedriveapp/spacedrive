import { useState } from 'react';
import { useNavigate } from 'react-router';
import { animated, useTransition } from 'react-spring';
import { getOnboardingStore, useBridgeMutation, useOnboardingStore } from '@sd/client';
import { Button, Card } from '@sd/ui';
import { Form, PasswordInput, useZodForm, z } from '@sd/ui/src/forms';
import { PasswordMeter } from '~/components/PasswordMeter';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './Layout';
import { useUnlockOnboardingScreen } from './Progress';

const schema = z.object({
	password: z.string(),
	password_validate: z.string(),
	algorithm: z.string(),
	hashing_algorithm: z.string()
});

const AnimatedCard = animated(Card);

const transitionConfig = {
	from: { opacity: 0, transform: `translateY(-40px)` },
	enter: { opacity: 1, transform: `translateY(0px)` },
	leave: { opacity: 0, transform: `translateY(-40px)` },
	config: { mass: 0.4, tension: 200, friction: 10, bounce: 0 }
};

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

	const obStore = useOnboardingStore();

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

	const pswTransition = useTransition(showPasswordValidate, transitionConfig);
	const pswErrTransition = useTransition(
		form.formState.errors.password_validate,
		transitionConfig
	);
	return (
		<Form form={form} onSubmit={onSubmit}>
			<OnboardingContainer>
				{/* <OnboardingImg src={Database} /> */}
				<OnboardingTitle>Set a master password</OnboardingTitle>
				<OnboardingDescription>
					This will be used to encrypt your library and/or open the built-in key manager.
				</OnboardingDescription>

				<div className="mt-4 flex w-[450px] flex-col">
					{pswErrTransition(
						(styles, pswValidateErr) =>
							pswValidateErr && (
								<AnimatedCard
									style={styles}
									className="mt-2 flex flex-col border-red-500/10 bg-red-500/20"
								>
									<span className="text-sm font-medium text-red-500">
										{pswValidateErr.message}
									</span>
								</AnimatedCard>
							)
					)}

					<div className="my-2 flex grow">
						<PasswordInput
							{...form.register('password')}
							size="lg"
							autoFocus
							tabIndex={1}
							className="w-full"
							disabled={form.formState.isSubmitting}
						/>
					</div>
					{pswTransition(
						(styles, show) =>
							show && (
								<animated.div style={styles} className="mb-2 flex grow">
									<PasswordInput
										{...form.register('password_validate')}
										size="lg"
										tabIndex={2}
										placeholder="Confirm password"
										autoFocus
										className="w-full"
										disabled={form.formState.isSubmitting}
									/>
								</animated.div>
							)
					)}

					<div className="mt-3 flex flex-col">
						<PasswordMeter password={form.watch('password')} />
					</div>
					<div className="mt-7 flex w-full justify-between">
						{!obStore.passwordSetToken ? (
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
								onClick={(event: any) => {
									// Without this, form is submitted before token gets removed
									event.preventDefault();
									getOnboardingStore().passwordSetToken = null;
									form.reset();
								}}
							>
								Remove password
							</Button>
						)}
						<Button
							tabIndex={3}
							disabled={form.formState.isSubmitting}
							type="submit"
							variant="accent"
							size="sm"
						>
							Set password
						</Button>
					</div>
				</div>
			</OnboardingContainer>
		</Form>
	);
}
