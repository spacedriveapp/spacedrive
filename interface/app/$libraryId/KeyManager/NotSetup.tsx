// import clsx from 'clsx';
// import { CaretRight, Spinner } from '@phosphor-icons/react';
// import { useState } from 'react';
// import {
// 	HASHING_ALGOS,
// 	hashingAlgoSlugSchema,
// 	useLibraryMutation,
// 	useLibraryQuery
// } from '@sd/client';
// import { Button, SelectOption, forms } from '@sd/ui';

// const { z, useZodForm, PasswordInput, Select, Form } = forms;

// const schema = z
// 	.object({
// 		password: z.string(),
// 		passwordValidate: z.string(),
// 		algorithm: z.enum(['XChaCha20Poly1305', 'Aes256Gcm']),
// 		hashingAlgorithm: hashingAlgoSlugSchema
// 	})
// 	.superRefine((data, ctx) => {
// 		if (!data.password || !data.passwordValidate) {
// 			ctx.addIssue({
// 				code: 'custom',
// 				path: ['password', 'passwordValidate'],
// 				message: 'Password is required'
// 			});
// 		}
// 		if (data.password && data.password !== data.passwordValidate) {
// 			ctx.addIssue({
// 				code: 'custom',
// 				path: ['passwordValidate'],
// 				message: 'Passwords do not match'
// 			});
// 		}
// 	});

// export default () => {
// 	const isSetup = useLibraryQuery(['keys.isSetup']);
// 	const setupKeyManager = useLibraryMutation('keys.setup');
// 	const [showAdvancedOptions, setShowAdvancedOptions] = useState(false);

// 	const form = useZodForm({
// 		schema: schema,
// 		defaultValues: {
// 			password: '',
// 			passwordValidate: '',
// 			algorithm: 'XChaCha20Poly1305',
// 			hashingAlgorithm: 'Argon2id-s'
// 		}
// 	});

// 	const onSubmit = form.handleSubmit((data) =>
// 		setupKeyManager
// 			.mutateAsync({
// 				password: data.password,
// 				algorithm: data.algorithm,
// 				hashing_algorithm: HASHING_ALGOS[data.hashingAlgorithm]
// 			})
// 			.then(() => isSetup.refetch())
// 	);

// 	const isSettingUp = setupKeyManager.isLoading || form.formState.isSubmitting;

// 	return (
// 		<Form form={form} onSubmit={onSubmit} className="w-[350px] p-4">
// 			<div className="space-y-4">
// 				<PasswordInput {...form.register('password')} label="Password" showStrength />

// 				<PasswordInput
// 					{...form.register('passwordValidate', {
// 						onChange: () => form.trigger('passwordValidate')
// 					})}
// 					label="Confirm password"
// 				/>

// 				<div className="rounded-md border border-app-line bg-app-overlay">
// 					<Button
// 						variant="bare"
// 						className={clsx(
// 							'flex w-full border-none !p-3',
// 							showAdvancedOptions && 'rounded-b-none'
// 						)}
// 						onClick={() => setShowAdvancedOptions(!showAdvancedOptions)}
// 					>
// 						Advanced Settings
// 						<CaretRight
// 							weight="bold"
// 							className={clsx('ml-1 transition', showAdvancedOptions && 'rotate-90')}
// 						/>
// 					</Button>

// 					{showAdvancedOptions && (
// 						<div className="space-y-4 p-3 pt-0">
// 							<div className="h-px bg-app-line" />
// 							<Select name="algorithm" label="Algorithm" size="lg" className="!mt-3">
// 								<SelectOption value="XChaCha20Poly1305">
// 									XChaCha20-Poly1305
// 								</SelectOption>
// 								<SelectOption value="Aes256Gcm">AES-256-GCM</SelectOption>
// 							</Select>

// 							<Select name="hashingAlgorithm" label="Hashing Algorithm" size="lg">
// 								<SelectOption value="Argon2id-s">Argon2id (standard)</SelectOption>
// 								<SelectOption value="Argon2id-h">Argon2id (hardened)</SelectOption>
// 								<SelectOption value="Argon2id-p">Argon2id (paranoid)</SelectOption>
// 								<SelectOption value="BalloonBlake3-s">
// 									BLAKE3-Balloon (standard)
// 								</SelectOption>
// 								<SelectOption value="BalloonBlake3-h">
// 									BLAKE3-Balloon (hardened)
// 								</SelectOption>
// 								<SelectOption value="BalloonBlake3-p">
// 									BLAKE3-Balloon (paranoid)
// 								</SelectOption>
// 							</Select>
// 						</div>
// 					)}
// 				</div>

// 				<Button
// 					type="submit"
// 					variant="accent"
// 					disabled={isSettingUp || !form.formState.isValid}
// 					className="w-full"
// 				>
// 					{isSettingUp ? (
// 						<Spinner className="mx-auto h-6 w-6 animate-spin fill-white text-white text-opacity-40" />
// 					) : (
// 						'Set up'
// 					)}
// 				</Button>
// 			</div>
// 		</Form>
// 	);
// };
