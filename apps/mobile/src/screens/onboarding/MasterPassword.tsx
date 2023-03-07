import React, { lazy, useState } from 'react';
import { Controller } from 'react-hook-form';
import { Alert, Text, View } from 'react-native';
import { getOnboardingStore, useBridgeMutation, useOnboardingStore } from '@sd/client';
import { PasswordInput } from '~/components/form/Input';
import { Button } from '~/components/primitive/Button';
import { useZodForm, z } from '~/hooks/useZodForm';
import { tw } from '~/lib/tailwind';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './GetStarted';

const PasswordMeter = lazy(() => import('~/components/key/PasswordMeter'));

const schema = z.object({
	password: z.string(),
	password_validate: z.string(),
	algorithm: z.string(),
	hashing_algorithm: z.string()
});

const MasterPasswordScreen = ({ navigation }: OnboardingStackScreenProps<'MasterPassword'>) => {
	const [showPasswordValidate, setShowPasswordValidate] = useState(false);

	const obStore = useOnboardingStore();

	const form = useZodForm({
		schema,
		defaultValues: {
			password: '',
			password_validate: '',
			algorithm: 'XChaCha20Poly1305',
			hashing_algorithm: 'Argon2id-s'
		}
	});

	const tokenizeSensitiveKey = useBridgeMutation('nodes.tokenizeSensitiveKey', {
		onSuccess: (data) => {
			getOnboardingStore().passwordSetToken = data.token;
			navigation.navigate('Privacy');
		},
		onError: (err: any) => {
			Alert.alert(err);
		}
	});

	const handleSetPassword = form.handleSubmit(async (data) => {
		if (data.password !== data.password_validate) {
			if (!showPasswordValidate) {
				setShowPasswordValidate(true);
				// focus on confirm password field
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

	const handleNoPassword = form.handleSubmit(async (data) => {
		tokenizeSensitiveKey.mutate({ secret_key: '' });
	});

	const password = form.watch('password');

	return (
		<OnboardingContainer>
			<OnboardingTitle>Set a master password</OnboardingTitle>
			<OnboardingDescription style={tw`mt-4`}>
				This will be used to encrypt your library and/or open the built-in key manager.
			</OnboardingDescription>
			<View style={tw`w-full`}>
				<View style={tw`mt-4 mb-2`}>
					<Controller
						control={form.control}
						name="password"
						render={({ field: { onBlur, onChange, value } }) => (
							<PasswordInput onChangeText={onChange} onBlur={onBlur} value={value} isNewPassword />
						)}
					/>
				</View>
				{showPasswordValidate && (
					<View style={tw`my-2`}>
						<Controller
							control={form.control}
							name="password_validate"
							render={({ field: { onBlur, onChange, value } }) => (
								<PasswordInput
									onChangeText={onChange}
									onBlur={onBlur}
									value={value}
									placeholder="Confirm password"
								/>
							)}
						/>
					</View>
				)}
				{form.formState.errors.password_validate && (
					<Text style={tw`my-2 text-center text-xs font-bold text-red-500`}>
						{form.formState.errors.password_validate.message}
					</Text>
				)}
				<PasswordMeter containerStyle={tw`mt-3 px-2`} password={form.watch('password')} />
				<View style={tw`mt-6`}>
					{obStore.passwordSetToken ? (
						<Button
							variant="outline"
							size="sm"
							disabled={form.formState.isSubmitting}
							onPress={() => {
								getOnboardingStore().passwordSetToken = null;
								form.reset();
							}}
						>
							<Text style={tw`text-ink text-center font-medium`}>Remove password</Text>
						</Button>
					) : (
						<Button
							variant="outline"
							size="sm"
							disabled={form.formState.isSubmitting}
							onPress={handleNoPassword}
						>
							<Text style={tw`text-ink text-center font-medium`}>Continue without password →</Text>
						</Button>
					)}
					{password.length > 0 && (
						<Button
							variant="outline"
							size="sm"
							disabled={form.formState.isSubmitting}
							onPress={handleSetPassword}
							style={tw`mt-4`}
						>
							<Text style={tw`text-ink text-center font-medium`}>Set password</Text>
						</Button>
					)}
				</View>
			</View>
		</OnboardingContainer>
	);
};

export default MasterPasswordScreen;
