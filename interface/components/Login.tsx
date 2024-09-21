import clsx from 'clsx';
import { Dispatch, SetStateAction, useEffect, useState } from 'react';
import { Controller } from 'react-hook-form';
import { useNavigate } from 'react-router';
import { signIn } from 'supertokens-web-js/recipe/emailpassword';
import { nonLibraryClient, useZodForm } from '@sd/client';
import { Button, Form, Input, toast, z } from '@sd/ui';
import { useIsDark, useLocale } from '~/hooks';

import ShowPassword from './ShowPassword';

async function signInClicked(
	email: string,
	password: string,
	reload: Dispatch<SetStateAction<boolean>>
) {
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
					toast.error(formField.error);
				}
			});
		} else if (response.status === 'WRONG_CREDENTIALS_ERROR') {
			toast.error('Email & password combination is incorrect.');
		} else if (response.status === 'SIGN_IN_NOT_ALLOWED') {
			toast.error(response.reason);
		} else {
			toast.success('Sign in successful');
			reload(true);
		}
	} catch (err: any) {
		if (err.isSuperTokensGeneralError === true) {
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

const Login = ({ reload }: { reload: Dispatch<SetStateAction<boolean>> }) => {
	const { t } = useLocale();
	const isDark = useIsDark();
	const [showPassword, setShowPassword] = useState(false);
	const navigate = useNavigate(); // useNavigate hook
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
				await signInClicked(data.email, data.password, reload);
			})}
			form={form}
		>
			<div className="flex flex-col gap-1.5">
				<div className="flex flex-col gap-4">
					<div className="flex flex-col">
						<label className="mb-2 text-left text-sm text-ink-dull">Email</label>
						<Controller
							control={form.control}
							name="email"
							render={({ field }) => (
								<Input
									{...field}
									placeholder="Enter your email address"
									error={Boolean(form.formState.errors.email?.message)}
									type="email"
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

					<div className="flex flex-col">
						<label className="mb-2 text-left text-sm text-ink-dull">Password</label>
						<Controller
							control={form.control}
							name="password"
							render={({ field }) => (
								<div className="relative flex items-center justify-center">
									<Input
										{...field}
										placeholder="Enter your password"
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
							<p className="text-xs text-red-500">
								{form.formState.errors.password.message}
							</p>
						)}
					</div>
				</div>

				{form.formState.errors.password && (
					<p className="text-xs text-red-500">{form.formState.errors.password.message}</p>
				)}
				<Button
					type="submit"
					className={clsx(
						'mx-auto mt-3 w-full border-none',
						isDark
							? [
									'mx-auto mt-3 w-full',
									'border-none bg-[#0E0E12]/30',
									'shadow-[0px_4px_30px_rgba(0,0,0,0.1)] backdrop-blur-lg backdrop-saturate-150',
									'rounded-lg px-4 py-2 text-white'
								]
							: ['text-black']
					)}
					variant={isDark ? 'default' : 'accent'}
					onClick={form.handleSubmit(async (data) => {
						await signInClicked(data.email, data.password, reload);
					})}
					disabled={form.formState.isSubmitting}
				>
					{t('login')}
				</Button>
			</div>
		</Form>
	);
};

export default Login;
