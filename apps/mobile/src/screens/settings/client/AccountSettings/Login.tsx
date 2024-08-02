import { useState } from 'react';
import { Controller } from 'react-hook-form';
import { Text, View } from 'react-native';
import { z } from 'zod';
import { useZodForm } from '@sd/client';
import { Button } from '~/components/primitive/Button';
import { Input } from '~/components/primitive/Input';
import { toast } from '~/components/primitive/Toast';
import { tw } from '~/lib/tailwind';
import { useNavigation } from '@react-navigation/native';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

async function signInClicked(email: string, password: string, navigator: SettingsStackScreenProps<'AccountProfile'>['navigation']) {
	try {
		const req = await fetch('http://localhost:9420/api/auth/signin', {
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
			toast.success('Sign in successful');
			// Refresh the page to show the user is logged in
			navigator.navigate('AccountProfile')
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

const LoginSchema = z.object({
	email: z.string().email(),
	password: z.string().min(6)
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
	const navigator = useNavigation<SettingsStackScreenProps<'AccountProfile'>['navigation']>();

	return (
		<View>
			<View style={tw`flex flex-col gap-1.5`}>
				<Controller
					control={form.control}
					name="email"
					render={({ field }) => (
						<Input
							{...field}
							placeholder="Email"
							style={tw`w-full`}
							onChangeText={field.onChange}
						/>
					)}
				/>
				{form.formState.errors.email && (
					<Text style={tw`text-xs text-red-500`}>
						{form.formState.errors.email.message}
					</Text>
				)}
				<Controller
					control={form.control}
					name="password"
					render={({ field }) => (
						<View style={tw`relative flex items-center justify-end`}>
							<Input
								{...field}
								placeholder="Password"
								style={tw`w-full`}
								onChangeText={field.onChange}
							/>
							{/* FIXME: Fix positioning of button */}
							{/* <ShowPassword
								showPassword={showPassword}
								setShowPassword={setShowPassword}
							/> */}
						</View>
					)}
				/>
				{form.formState.errors.password && (
					<Text style={tw`text-xs text-red-500`}>
						{form.formState.errors.password.message}
					</Text>
				)}
				<Button
					style={tw`mx-auto mt-2 w-full`}
					variant="accent"
					onPress={form.handleSubmit(async (data) => {
						await signInClicked(data.email, data.password, navigator);
					})}
					disabled={form.formState.isSubmitting}
				>
					<Text>Submit</Text>
				</Button>
			</View>
		</View>
	);
};

export default Login;
