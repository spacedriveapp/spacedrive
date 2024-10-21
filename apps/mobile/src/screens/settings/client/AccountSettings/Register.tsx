import { zodResolver } from '@hookform/resolvers/zod';
import { useNavigation } from '@react-navigation/native';
import { useState } from 'react';
import { Controller, useForm } from 'react-hook-form';
import { Text, View } from 'react-native';
import { z } from 'zod';
import { Button } from '~/components/primitive/Button';
import { Input } from '~/components/primitive/Input';
import { toast } from '~/components/primitive/Toast';
import { tw, twStyle } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';
import { AUTH_SERVER_URL } from '~/utils';

import ShowPassword from './ShowPassword';

const RegisterSchema = z
	.object({
		email: z.string().email({
			message: 'Email is required'
		}),
		password: z.string().min(6, {
			message: 'Password must be at least 6 characters'
		}),
		confirmPassword: z.string().min(6, {
			message: 'Password must be at least 6 characters'
		})
	})
	.refine((data) => data.password === data.confirmPassword, {
		message: 'Passwords do not match',
		path: ['confirmPassword']
	});
type RegisterType = z.infer<typeof RegisterSchema>;

const Register = () => {
	const [showPassword, setShowPassword] = useState(false);
	// useZodForm seems to be out-dated or needs
	//fixing as it does not support the schema using zod.refine
	const form = useForm<RegisterType>({
		resolver: zodResolver(RegisterSchema),
		defaultValues: {
			email: '',
			password: '',
			confirmPassword: ''
		}
	});

	const navigator = useNavigation<SettingsStackScreenProps<'AccountProfile'>['navigation']>();
	return (
		<View style={tw`flex flex-col gap-1.5`}>
			<Controller
				control={form.control}
				name="email"
				render={({ field }) => (
					<Input
						{...field}
						style={twStyle(`w-full`, form.formState.errors.email && 'border-red-500')}
						placeholder="Email"
						onChangeText={field.onChange}
					/>
				)}
			/>
			{form.formState.errors.email && (
				<Text style={tw`text-xs text-red-500`}>{form.formState.errors.email.message}</Text>
			)}
			<Controller
				control={form.control}
				name="password"
				render={({ field }) => (
					<View style={tw`relative flex items-center justify-center`}>
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
					</View>
				)}
			/>
			{form.formState.errors.password && (
				<Text style={tw`text-xs text-red-500`}>
					{form.formState.errors.password.message}
				</Text>
			)}
			<Controller
				control={form.control}
				name="confirmPassword"
				render={({ field }) => (
					<View style={tw`relative flex items-start`}>
						<Input
							{...field}
							placeholder="Confirm Password"
							style={twStyle(
								`w-full`,
								form.formState.errors.confirmPassword && 'border-red-500'
							)}
							onChangeText={field.onChange}
							secureTextEntry={!showPassword}
						/>
						{form.formState.errors.confirmPassword && (
							<Text style={tw`my-1 text-xs text-red-500`}>
								{form.formState.errors.confirmPassword.message}
							</Text>
						)}
						<ShowPassword
							showPassword={showPassword}
							setShowPassword={setShowPassword}
							plural={true}
						/>
					</View>
				)}
			/>
			<Button
				style={tw`mx-auto mt-2 w-full`}
				variant="accent"
				onPress={form.handleSubmit(
					async (data) => await signUpClicked(data.email, data.password, navigator)
				)}
				disabled={form.formState.isSubmitting}
			>
				<Text style={tw`font-bold text-white`}>Submit</Text>
			</Button>
		</View>
	);
};

async function signUpClicked(
	email: string,
	password: string,
	navigator: SettingsStackScreenProps<'AccountProfile'>['navigation']
) {
	try {
		const req = await fetch(`${AUTH_SERVER_URL}/api/auth/signup`, {
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
			// one of the input formFields failed validaiton
			console.error('Field error: ', response.reason);
		} else if (response.status === 'SIGN_UP_NOT_ALLOWED') {
			// the reason string is a user friendly message
			// about what went wrong. It can also contain a support code which users
			// can tell you so you know why their sign up was not allowed.
			toast.error(response.reason!);
		} else {
			// sign up successful. The session tokens are automatically handled by
			// the frontend SDK.
			toast.success('Sign up successful');
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

export default Register;
