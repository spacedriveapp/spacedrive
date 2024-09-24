import { AlphaRSPCError } from '@oscartbeaumont-sd/rspc-client/v2';
import { UseMutationResult } from '@tanstack/react-query';
import clsx from 'clsx';
import { Dispatch, SetStateAction, useState } from 'react';
import { Controller } from 'react-hook-form';
import { signIn } from 'supertokens-web-js/recipe/emailpassword';
import { useZodForm } from '@sd/client';
import { Button, Form, Input, toast, z } from '@sd/ui';
import { useLocale } from '~/hooks';
import { getTokens } from '~/util';

import ShowPassword from './ShowPassword';

async function signInClicked(
	email: string,
	password: string,
	reload: Dispatch<SetStateAction<boolean>>,
	cloudBootstrap: UseMutationResult<null, AlphaRSPCError, [string, string], unknown> // Cloud bootstrap mutation
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
			const tokens = getTokens();
			console.log(cloudBootstrap);
			cloudBootstrap.mutate([tokens.accessToken, tokens.refreshToken]);
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

const Login = ({
	reload,
	cloudBootstrap
}: {
	reload: Dispatch<SetStateAction<boolean>>;
	cloudBootstrap: UseMutationResult<null, AlphaRSPCError, [string, string], unknown>; // Cloud bootstrap mutation
}) => {
	const { t } = useLocale();
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
				await signInClicked(data.email, data.password, reload, cloudBootstrap);
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
								error={Boolean(form.formState.errors.email?.message)}
								type="email"
								disabled={form.formState.isSubmitting}
								className="w-full"
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
							<div className="relative flex w-full items-center justify-center">
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
						<p className="text-xs text-red-500">
							{form.formState.errors.password.message}
						</p>
					)}
				</div>
			</div>
			<Button
				type="submit"
				className={clsx('mx-auto mt-3 w-full border-none')}
				variant="accent"
				onClick={form.handleSubmit(async (data) => {
					await signInClicked(data.email, data.password, reload, cloudBootstrap);
				})}
				disabled={form.formState.isSubmitting}
			>
				{t('login')}
			</Button>
		</Form>
	);
};

export default Login;
