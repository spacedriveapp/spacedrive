import AsyncStorage from '@react-native-async-storage/async-storage';
import { useNavigation } from '@react-navigation/native';
import { RSPCError } from '@spacedrive/rspc-client';
import { UseMutationResult } from '@tanstack/react-query';
import { useState } from 'react';
import { Controller } from 'react-hook-form';
import { Text, View } from 'react-native';
import { z } from 'zod';
import { useBridgeMutation, useZodForm } from '@sd/client';
import { Button } from '~/components/primitive/Button';
import { Input } from '~/components/primitive/Input';
import { toast } from '~/components/primitive/Toast';
import { tw, twStyle } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';
import { getUserStore } from '~/stores/userStore';
import { AUTH_SERVER_URL } from '~/utils';

import ShowPassword from './ShowPassword';

const LoginSchema = z.object({
	email: z.string().email({
		message: 'Email is required'
	}),
	password: z.string().min(6, {
		message: 'Password must be at least 6 characters'
	})
});

const Login = () => {
	const [showPassword, setShowPassword] = useState(false);
	const form = useZodForm({
		schema: LoginSchema,
		defaultValues: {
			email: '',
			password: ''
		}
	});
	const updateUserStore = getUserStore();
	const navigator = useNavigation<SettingsStackScreenProps<'AccountProfile'>['navigation']>();
	const cloudBootstrap = useBridgeMutation('cloud.bootstrap');

	return (
		<View>
			<View style={tw`flex flex-col gap-1.5`}>
				<Controller
					control={form.control}
					name="email"
					render={({ field }) => (
						<View style={tw`relative flex items-start`}>
							<Input
								{...field}
								placeholder="Email"
								style={twStyle(
									`w-full`,
									form.formState.errors.email && 'border-red-500'
								)}
								onChangeText={field.onChange}
							/>
							{form.formState.errors.email && (
								<Text style={tw`my-1 text-xs text-red-500`}>
									{form.formState.errors.email.message}
								</Text>
							)}
						</View>
					)}
				/>
				<Controller
					control={form.control}
					name="password"
					render={({ field }) => (
						<View style={tw`relative flex items-start`}>
							<Input
								{...field}
								placeholder="Password"
								style={twStyle(
									`w-full`,
									form.formState.errors.password && 'border-red-500'
								)}
								onChangeText={field.onChange}
								secureTextEntry={!showPassword}
							/>
							{form.formState.errors.password && (
								<Text style={tw`my-1 text-xs text-red-500`}>
									{form.formState.errors.password.message}
								</Text>
							)}
							<ShowPassword
								showPassword={showPassword}
								setShowPassword={setShowPassword}
							/>
						</View>
					)}
				/>
				<Button
					style={tw`mx-auto mt-2 w-full`}
					variant="accent"
					onPress={form.handleSubmit(async (data) => {
						await signInClicked(
							data.email,
							data.password,
							navigator,
							cloudBootstrap,
							updateUserStore
						);
					})}
					disabled={form.formState.isSubmitting}
				>
					<Text style={tw`font-bold text-white`}>Submit</Text>
				</Button>
			</View>
		</View>
	);
};

async function signInClicked(
	email: string,
	password: string,
	navigator: SettingsStackScreenProps<'AccountProfile'>['navigation'],
	cloudBootstrap: UseMutationResult<null, RSPCError, [string, string], unknown>, // Cloud bootstrap mutation
	updateUserStore: ReturnType<typeof getUserStore>
) {
	try {
		const req = await fetch(`${AUTH_SERVER_URL}/api/auth/signin`, {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json; charset=utf-8'
			},
			body: JSON.stringify({
				formFields: [
					{
						id: 'email',
						value: email
					},
					{
						id: 'password',
						value: password
					}
				]
			})
		});

		const response: {
			status: string;
			reason?: string;
			user?: {
				id: string;
				email: string;
				timeJoined: number;
				tenantIds: string[];
			};
		} = await req.json();

		if (response.status === 'FIELD_ERROR') {
			// response.reason?.forEach((formField) => {
			// 	if (formField.id === 'email') {
			// 		// Email validation failed (for example incorrect email syntax).
			// 		toast.error(formField.error);
			// 	}
			// });
			console.error('Field error: ', response.reason);
		} else if (response.status === 'WRONG_CREDENTIALS_ERROR') {
			toast.error('Email & password combination is incorrect.');
		} else if (response.status === 'SIGN_IN_NOT_ALLOWED') {
			// the reason string is a user friendly message
			// about what went wrong. It can also contain a support code which users
			// can tell you so you know why their sign in was not allowed.
			toast.error(response.reason!);
		} else {
			// sign in successful. The session tokens are automatically handled by
			// the frontend SDK.
			cloudBootstrap.mutate([
				req.headers.get('st-access-token')!,
				req.headers.get('st-refresh-token')!
			]);
			toast.success('Sign in successful');
			// Update the user store with the user info
			updateUserStore.userInfo = response.user;
			// Save the access token to AsyncStorage, because SuperTokens doesn't store it correctly. Thanks to the React Native SDK.
			await AsyncStorage.setItem('access_token', req.headers.get('st-access-token')!);
			await AsyncStorage.setItem('refresh_token', req.headers.get('st-refresh-token')!);
			// Refresh the page to show the user is logged in
			navigator.navigate('AccountProfile');
		}
	} catch (err: any) {
		if (err.isSuperTokensGeneralError === true) {
			// this may be a custom error message sent from the API by you.
			toast.error(err.message);
		} else {
			console.error(err);
			toast.error('Oops! Something went wrong.');
		}
	}
}

export default Login;
