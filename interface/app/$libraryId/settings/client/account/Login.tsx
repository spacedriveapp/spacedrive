import { useEffect, useState } from 'react';
import { Controller } from 'react-hook-form';
import { signIn } from 'supertokens-web-js/recipe/emailpassword';
import { nonLibraryClient, useZodForm } from '@sd/client';
import { Button, Form, Input, toast, z } from '@sd/ui';
import ShowPassword from './ShowPassword';

async function signInClicked(email: string, password: string) {
	try {
		const response = await signIn({
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
		});

		if (response.status === 'FIELD_ERROR') {
			response.formFields.forEach((formField) => {
				if (formField.id === 'email') {
					// Email validation failed (for example incorrect email syntax).
					toast.error(formField.error);
				}
			});
		} else if (response.status === 'WRONG_CREDENTIALS_ERROR') {
			toast.error('Email & password combination is incorrect.');
		} else if (response.status === 'SIGN_IN_NOT_ALLOWED') {
			// the reason string is a user friendly message
			// about what went wrong. It can also contain a support code which users
			// can tell you so you know why their sign in was not allowed.
			toast.error(response.reason);
		} else {
			// sign in successful. The session tokens are automatically handled by
			// the frontend SDK.
			console.log('Sign in successful');
		}
	} catch (err: any) {
		if (err.isSuperTokensGeneralError === true) {
			// this may be a custom error message sent from the API by you.
			toast.error(err.message);
		} else {
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

	return (
		<Form
			onSubmit={form.handleSubmit(async (data) => {
				// handle login submission
				await signInClicked(data.email, data.password);
			})}
			form={form}
		>
			<div className="flex flex-col gap-1.5">
				<Controller
					control={form.control}
					name="email"
					render={({ field }) => (
						<Input
							{...field}
							placeholder="Email"
							error={Boolean(form.formState.errors.email?.message)}
							type="email"
							disabled={form.formState.isSubmitting}
						/>
					)}
				/>
				{form.formState.errors.email && (
					<p className="text-xs text-red-500">{form.formState.errors.email.message}</p>
				)}
				<Controller
					control={form.control}
					name="password"
					render={({ field }) => (
						<div className="relative flex items-center justify-center">
							<Input
								{...field}
								placeholder="Password"
								error={Boolean(form.formState.errors.password?.message)}
								className="w-full"
								disabled={form.formState.isSubmitting}
								type={showPassword ? 'text' : 'password'}
								onPaste={(e) => {
									const pastedText = e.clipboardData.getData('text');
									field.onChange(pastedText);
								}}
							/>
							<ShowPassword
								showPassword={showPassword}
								setShowPassword={setShowPassword}
							/>
						</div>
					)}
				/>
				{form.formState.errors.password && (
					<p className="text-xs text-red-500">{form.formState.errors.password.message}</p>
				)}
				<Button
					type="submit"
					className="mx-auto mt-2 w-full"
					variant="accent"
					onClick={form.handleSubmit(async (data) => {
						await signInClicked(data.email, data.password);
					})}
					disabled={form.formState.isSubmitting}
				>
					Submit
				</Button>
			</div>
		</Form>
	);
};

export default Login;
