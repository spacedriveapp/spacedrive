import { zodResolver } from '@hookform/resolvers/zod';
import clsx from 'clsx';
import { useState } from 'react';
import { Controller, useForm } from 'react-hook-form';
import { signUp } from 'supertokens-web-js/recipe/emailpassword';
import { Button, Form, Input, toast, z } from '@sd/ui';
import { useLocale } from '~/hooks';

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

async function signUpClicked(email: string, password: string) {
	try {
		const response = await signUp({
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
			// one of the input formFields failed validaiton
			response.formFields.forEach((formField) => {
				if (formField.id === 'email') {
					// Email validation failed (for example incorrect email syntax),
					// or the email is not unique.
					toast.error(formField.error);
				} else if (formField.id === 'password') {
					// Password validation failed.
					// Maybe it didn't match the password strength
					toast.error(formField.error);
				}
			});
		} else if (response.status === 'SIGN_UP_NOT_ALLOWED') {
			// the reason string is a user friendly message
			// about what went wrong. It can also contain a support code which users
			// can tell you so you know why their sign up was not allowed.
			toast.error(response.reason);
		} else {
			// sign up successful. The session tokens are automatically handled by
			// the frontend SDK.
			toast.success('Sign up successful');
			// FIXME: This is a temporary workaround. We will provide a better way to handle this.
			window.location.reload();
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

const Register = () => {
	const { t } = useLocale();
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
	return (
		<Form
			onSubmit={form.handleSubmit(async (data) => {
				// handle sign-up submission
				console.log(data);
				await signUpClicked(data.email, data.password);
			})}
			className="w-full"
			form={form}
		>
			<div className="flex flex-col gap-3">
				<div className="flex flex-col items-start gap-1">
					<label className="text-left text-sm text-ink-dull">Email</label>
					<Controller
						control={form.control}
						name="email"
						render={({ field }) => (
							<Input
								{...field}
								placeholder="johndoe@gmail.com"
								className="w-full"
								error={Boolean(form.formState.errors.email?.message)}
								type="email"
								size="md"
								disabled={form.formState.isSubmitting}
							/>
						)}
					/>
					{form.formState.errors.email && (
						<p className="text-xs text-red-500">
							{form.formState.errors.email.message}
						</p>
					)}
				</div>

				<div className="flex flex-col items-start gap-1">
					<label className="text-left text-sm text-ink-dull">Password</label>
					<Controller
						control={form.control}
						name="password"
						render={({ field }) => (
							<div className="relative flex w-full items-start">
								<Input
									{...field}
									placeholder="Password"
									error={Boolean(form.formState.errors.password?.message)}
									className="w-full"
									size="md"
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
						<p className="text-xs text-red-500">
							{form.formState.errors.password.message}
						</p>
					)}
					<Controller
						control={form.control}
						name="confirmPassword"
						render={({ field }) => (
							<div className="relative mt-0.5 flex w-full items-start">
								<Input
									{...field}
									placeholder="Confirm password"
									size="md"
									error={Boolean(form.formState.errors.confirmPassword?.message)}
									className="w-full"
									disabled={form.formState.isSubmitting}
									type={showPassword ? 'text' : 'password'}
								/>
								<ShowPassword
									showPassword={showPassword}
									setShowPassword={setShowPassword}
								/>
							</div>
						)}
					/>
					{form.formState.errors.confirmPassword && (
						<p className="text-xs text-red-500">
							{form.formState.errors.confirmPassword.message}
						</p>
					)}
				</div>
			</div>
			<Button
				type="submit"
				className={clsx('mx-auto mt-3 w-full border-none')}
				size="md"
				variant="accent"
				onClick={form.handleSubmit(async (data) => {
					console.log(data);
					await signUpClicked(data.email, data.password);
				})}
				disabled={form.formState.isSubmitting}
			>
				{t('register')}
			</Button>
		</Form>
	);
};

export default Register;
