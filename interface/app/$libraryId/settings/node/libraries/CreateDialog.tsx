import { useQueryClient } from '@tanstack/react-query';
import clsx from 'clsx';
import { CaretRight } from 'phosphor-react';
import { useEffect, useState } from 'react';
import { Controller } from 'react-hook-form';
import { useNavigate } from 'react-router-dom';
import {
	HASHING_ALGOS,
	LibraryConfigWrapped,
	hashingAlgoSlugSchema,
	useBridgeMutation,
	usePlausibleEvent
} from '@sd/client';
import {
	Button,
	Dialog,
	RadixCheckbox,
	SelectOption,
	UseDialogProps,
	forms,
	useDialog
} from '@sd/ui';

const { Input, z, useZodForm, PasswordInput, Select } = forms;

const schema = z
	.object({
		name: z.string().min(1),
		encryptLibrary: z.boolean(),
		password: z.string(),
		passwordValidate: z.string(),
		algorithm: z.enum(['XChaCha20Poly1305', 'Aes256Gcm']),
		hashingAlgorithm: hashingAlgoSlugSchema
	})
	.superRefine((data, ctx) => {
		if (data.encryptLibrary && !data.password) {
			ctx.addIssue({
				code: 'custom',
				path: ['password'],
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

export default (props: UseDialogProps) => {
	const dialog = useDialog(props);
	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const submitPlausibleEvent = usePlausibleEvent();

	const [showAdvancedOptions, setShowAdvancedOptions] = useState(false);

	const createLibrary = useBridgeMutation('library.create', {
		onSuccess: (library) => {
			queryClient.setQueryData(
				['library.list'],
				(libraries: LibraryConfigWrapped[] | undefined) => [...(libraries || []), library]
			);

			submitPlausibleEvent({
				event: {
					type: 'libraryCreate'
				}
			});

			navigate(`/${library.uuid}/overview`);
		},
		onError: (err) => console.log(err)
	});

	const form = useZodForm({
		schema: schema,
		defaultValues: {
			encryptLibrary: false,
			password: '',
			passwordValidate: '',
			algorithm: 'XChaCha20Poly1305',
			hashingAlgorithm: 'Argon2id-s'
		}
	});

	const onSubmit = form.handleSubmit(async (data) => {
		await createLibrary.mutateAsync({
			name: data.name,
			algorithm: data.algorithm,
			hashing_algorithm: HASHING_ALGOS[data.hashingAlgorithm],
			auth: {
				type: 'Password',
				value: data.encryptLibrary ? data.password : ''
			}
		});
	});

	const encryptLibrary = form.watch('encryptLibrary');

	useEffect(() => {
		if (showAdvancedOptions) setShowAdvancedOptions(false);
	}, [encryptLibrary]);

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			dialog={dialog}
			submitDisabled={!form.formState.isValid}
			title="Create New Library"
			description="Libraries are a secure, on-device database. Your files remain where they are, the Library catalogs them and stores all Spacedrive related data."
			ctaLabel={form.formState.isSubmitting ? 'Creating library...' : 'Create library'}
		>
			<div className="space-y-4">
				<Input
					{...form.register('name')}
					label="Library name"
					placeholder={'e.g. "James\' Library"'}
					size="md"
				/>

				<Controller
					control={form.control}
					name="encryptLibrary"
					render={({ field }) => (
						<RadixCheckbox
							checked={field.value}
							onCheckedChange={field.onChange}
							label="Encrypt Library"
							name="encryptLibrary"
						/>
					)}
				/>

				{encryptLibrary && (
					<>
						<div className="border-b border-app-line" />

						<PasswordInput
							{...form.register('password')}
							label="Password"
							showStrength
						/>
						<PasswordInput
							{...form.register('passwordValidate', {
								onBlur: () => form.trigger('passwordValidate')
							})}
							label="Confirm password"
						/>

						<div className="rounded-md border border-app-line bg-app-overlay">
							<Button
								variant="bare"
								className={clsx(
									'flex w-full border-none !p-3',
									showAdvancedOptions && 'rounded-b-none'
								)}
								onClick={() => setShowAdvancedOptions(!showAdvancedOptions)}
							>
								Advanced Settings
								<CaretRight
									weight="bold"
									className={clsx(
										'ml-1 transition',
										showAdvancedOptions && 'rotate-90'
									)}
								/>
							</Button>

							{showAdvancedOptions && (
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
										<SelectOption value="Argon2id-s">
											Argon2id (standard)
										</SelectOption>
										<SelectOption value="Argon2id-h">
											Argon2id (hardened)
										</SelectOption>
										<SelectOption value="Argon2id-p">
											Argon2id (paranoid)
										</SelectOption>
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
							)}
						</div>
					</>
				)}
			</div>
		</Dialog>
	);
};
