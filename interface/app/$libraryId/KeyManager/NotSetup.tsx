import { HASHING_ALGOS, hashingAlgoSlugSchema, useLibraryMutation } from '@sd/client';
import { Button, SelectOption, forms } from '@sd/ui';
import { Form } from '~/../packages/ui/src/forms';

const { z, useZodForm, PasswordInput, Select } = forms;

const schema = z
	.object({
		password: z.string(),
		passwordValidate: z.string(),
		algorithm: z.enum(['XChaCha20Poly1305', 'Aes256Gcm']),
		hashingAlgorithm: hashingAlgoSlugSchema
	})
	.superRefine((data, ctx) => {
		if (!data.password || !data.passwordValidate) {
			ctx.addIssue({
				code: 'custom',
				path: ['password', 'passwordValidate'],
				message: 'Password is required'
			});
		}
		if (data.password && data.password !== data.passwordValidate) {
			ctx.addIssue({
				code: 'custom',
				path: ['passwordValidate'],
				message: 'Passwords do not match'
			});
		}
	});

export default () => {
	const setupKeyManager = useLibraryMutation('keys.setup');

	const form = useZodForm({
		schema: schema,
		defaultValues: {
			password: '',
			passwordValidate: '',
			algorithm: 'XChaCha20Poly1305',
			hashingAlgorithm: 'Argon2id-s'
		}
	});

	const onSubmit = form.handleSubmit(async (data) => {
		await setupKeyManager.mutateAsync({
			password: data.password,
			algorithm: data.algorithm,
			hashing_algorithm: HASHING_ALGOS[data.hashingAlgorithm]
		});
	});

	return (
		<Form form={form} onSubmit={onSubmit}>
			<div className="mt-5 space-y-4">
				<PasswordInput {...form.register('password')} label="Password" showStrength />
				<PasswordInput
					{...form.register('passwordValidate', {
						onBlur: () => form.trigger('passwordValidate')
					})}
					label="Confirm password"
				/>

				<div className="rounded-md border border-app-line bg-app-overlay">
					<div className="space-y-4 p-3 pt-0">
						<div className="h-px bg-app-line" />
						<Select
							control={form.control}
							name="algorithm"
							label="Algorithm"
							size="md"
							className="!mt-3"
						>
							<SelectOption value="XChaCha20Poly1305">
								XChaCha20-Poly1305
							</SelectOption>
							<SelectOption value="Aes256Gcm">AES-256-GCM</SelectOption>
						</Select>

						<Select
							control={form.control}
							name="hashingAlgorithm"
							label="Hashing Algorithm"
							size="md"
						>
							<SelectOption value="Argon2id-s">Argon2id (standard)</SelectOption>
							<SelectOption value="Argon2id-h">Argon2id (hardened)</SelectOption>
							<SelectOption value="Argon2id-p">Argon2id (paranoid)</SelectOption>
							<SelectOption value="BalloonBlake3-s">
								BLAKE3-Balloon (standard)
							</SelectOption>
							<SelectOption value="BalloonBlake3-h">
								BLAKE3-Balloon (hardened)
							</SelectOption>
							<SelectOption value="BalloonBlake3-p">
								BLAKE3-Balloon (paranoid)
							</SelectOption>
						</Select>
					</div>
				</div>

				<Button
					className="w-full"
					variant="accent"
					disabled={!form.formState.isValid}
					type="submit"
				>
					Set up
				</Button>
			</div>
		</Form>
	);
};
