import { ArrowLeft } from '@phosphor-icons/react';
import { RSPCError } from '@spacedrive/rspc-client';
import { UseMutationResult } from '@tanstack/react-query';
import clsx from 'clsx';
import { Dispatch, SetStateAction, useState } from 'react';
import { Controller } from 'react-hook-form';
import { signIn } from 'supertokens-web-js/recipe/emailpassword';
import { createCode } from 'supertokens-web-js/recipe/passwordless';
import { useZodForm } from '@sd/client';
import { Button, Divider, Form, Input, toast, z } from '@sd/ui';
import { useLocale } from '~/hooks';
import { getTokens } from '~/util';

import ShowPassword from './ShowPassword';

async function signInClicked(
	email: string,
	password: string,
	reload: Dispatch<SetStateAction<boolean>>,
	cloudBootstrap: UseMutationResult<null, RSPCError, [string, string], unknown> // Cloud bootstrap mutation
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
			console.error('Error signing in', err);
			toast.error('Oops! Something went wrong.');
		}
	}
}

const LoginSchema = z.object({
	email: z.string().email({
		message: 'Email is required'
	}),
	password: z.string().min(6, {
		message: 'Password must be at least 6 characters'
	})
});

const ContinueWithEmailSchema = z.object({
	email: z.string().email({
		message: 'Email is required'
	})
});

const Login = ({
	reload,
	cloudBootstrap
}: {
	reload: Dispatch<SetStateAction<boolean>>;
	cloudBootstrap: UseMutationResult<null, RSPCError, [string, string], unknown>; // Cloud bootstrap mutation
}) => {
	const [continueWithEmail, setContinueWithEmail] = useState(false);

	return (
		<>
			{continueWithEmail ? (
				<ContinueWithEmail
					setContinueWithEmail={setContinueWithEmail}
					reload={reload}
					cloudBootstrap={cloudBootstrap}
				/>
			) : (
				<LoginForm
					setContinueWithEmail={setContinueWithEmail}
					reload={reload}
					cloudBootstrap={cloudBootstrap}
				/>
			)}
		</>
	);
};

interface LoginProps {
	reload: Dispatch<SetStateAction<boolean>>;
	cloudBootstrap: UseMutationResult<null, RSPCError, [string, string], unknown>; // Cloud bootstrap mutation
	setContinueWithEmail: Dispatch<SetStateAction<boolean>>;
}

const LoginForm = ({ reload, cloudBootstrap, setContinueWithEmail }: LoginProps) => {
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
								size="md"
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
									size="md"
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
				size="md"
				onClick={form.handleSubmit(async (data) => {
					await signInClicked(data.email, data.password, reload, cloudBootstrap);
				})}
				disabled={form.formState.isSubmitting}
			>
				{t('login')}
			</Button>

			<div className="my-3 flex items-center gap-4">
				<Divider className="bg-app-line/90" />
				<p className="text-xs font-medium uppercase text-ink-faint">Or</p>
				<Divider className="bg-app-line/90" />
			</div>

			<Button
				variant="gray"
				className="w-full"
				size="md"
				onClick={() => {
					form.reset();
					setContinueWithEmail(true);
				}}
				disabled={form.formState.isSubmitting}
			>
				Continue with email
			</Button>
		</Form>
	);
};

interface Props {
	setContinueWithEmail: Dispatch<SetStateAction<boolean>>;
	reload: Dispatch<SetStateAction<boolean>>;
	cloudBootstrap: UseMutationResult<null, RSPCError, [string, string], unknown>; // Cloud bootstrap mutation
}

const ContinueWithEmail = ({ setContinueWithEmail, reload, cloudBootstrap }: Props) => {
	const { t } = useLocale();
	const ContinueWithEmailForm = useZodForm({
		schema: ContinueWithEmailSchema,
		defaultValues: {
			email: ''
		}
	});
	const [step, setStep] = useState(1);

	return (
		<Form
			onSubmit={ContinueWithEmailForm.handleSubmit(async (data) => {
				// send email
				await sendMagicLink(data.email);
				setStep((step) => step + 1);
			})}
			className="w-full"
			form={ContinueWithEmailForm}
		>
			{step === 1 ? (
				<>
					<div className="flex flex-col items-start gap-1">
						<label className="text-left text-sm text-ink-dull">Email</label>
						<Controller
							control={ContinueWithEmailForm.control}
							name="email"
							render={({ field }) => (
								<Input
									{...field}
									type="email"
									size="md"
									className="w-full"
									placeholder="johndoe@gmail.com"
									error={Boolean(
										ContinueWithEmailForm.formState.errors.email?.message
									)}
								/>
							)}
						/>
						{ContinueWithEmailForm.formState.errors.email && (
							<p className="text-xs text-red-500">
								{ContinueWithEmailForm.formState.errors.email.message}
							</p>
						)}
					</div>
					<Button
						type="submit"
						size="md"
						className="mx-auto mt-3 w-full border-none"
						variant="accent"
						onClick={() => {}}
						disabled={ContinueWithEmailForm.formState.isSubmitting}
					>
						{t('continue')}
					</Button>
				</>
			) : (
				<div className="flex flex-col gap-1.5">
					<p className="text-lg font-bold">Check your email</p>
					<div className="flex flex-col">
						<p>{t('login_link_sent')}</p>
						<p>
							{t('check_your_inbox')}{' '}
							<span className="font-bold">
								{ContinueWithEmailForm.getValues().email}
							</span>
						</p>
					</div>
				</div>
			)}
			<Button
				variant="subtle"
				size="md"
				className="mt-5 flex w-full justify-center gap-1.5"
				onClick={() => {
					if (step === 2) return setStep(1);
					ContinueWithEmailForm.reset();
					setContinueWithEmail(false);
				}}
				disabled={ContinueWithEmailForm.formState.isSubmitting}
			>
				<ArrowLeft />
				{t('back_to_login')}
			</Button>
		</Form>
	);
};

async function sendMagicLink(email: string) {
	try {
		const response = await createCode({
			email
		});

		if (response.status === 'SIGN_IN_UP_NOT_ALLOWED') {
			// the reason string is a user friendly message
			// about what went wrong. It can also contain a support code which users
			// can tell you so you know why their sign in / up was not allowed.
			toast.error(response.reason);
		}
	} catch (err: any) {
		if (err.isSuperTokensGeneralError === true) {
			// this may be a custom error message sent from the API by you,
			// or if the input email / phone number is not valid.
			toast.error(err.message);
		} else {
			console.error(err);
			toast.error('Oops! Something went wrong.');
		}
	}
}

export default Login;
