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
		schema: z
			.object({
				name: z.string().min(1),
				encrypt_library: z.boolean(),
				password: z.string(),
				password_validate: z.string(),
				algorithm: z.enum(['XChaCha20Poly1305', 'Aes256Gcm']),
				hashing_algorithm: hashingAlgoSlugSchema
			})
			.superRefine((data, ctx) => {
				if (data.encrypt_library && !data.password) {
					ctx.addIssue({
						code: 'custom',
						path: ['password'],
						message: 'Password is required'
					});
				}
				if (data.password && data.password !== data.password_validate) {
					ctx.addIssue({
						code: 'custom',
						path: ['password_validate'],
						message: 'Passwords do not match'
					});
				}
			}),
		defaultValues: {
			encrypt_library: false,
			password: '',
			password_validate: '',
			algorithm: 'XChaCha20Poly1305',
			hashing_algorithm: 'Argon2id-s'
		}
	});

	const onSubmit = form.handleSubmit(async (data) => {
		await createLibrary.mutateAsync({
			name: data.name,
			algorithm: data.algorithm,
			hashing_algorithm: HASHING_ALGOS[data.hashing_algorithm],
			auth: {
				type: 'Password',
				value: data.encrypt_library ? data.password : ''
			}
		});
	});

	const encryptLibrary = form.watch('encrypt_library');

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
					name="encrypt_library"
					render={({ field }) => (
						<RadixCheckbox
							checked={field.value}
							onCheckedChange={field.onChange}
							label="Encrypt Library"
							name="encrypt_library"
						/>
					)}
				/>

				{encryptLibrary && (
					<>
						<div className="border-b border-app-line" />

						<PasswordInput {...form.register('password')} label="Password" showStrength />
						<PasswordInput
							{...form.register('password_validate', {
								onBlur: () => form.trigger('password_validate')
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
									className={clsx('ml-1 transition', showAdvancedOptions && 'rotate-90')}
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
										<SelectOption value="XChaCha20Poly1305">XChaCha20-Poly1305</SelectOption>
										<SelectOption value="Aes256Gcm">AES-256-GCM</SelectOption>
									</Select>

									<Select
										control={form.control}
										name="hashing_algorithm"
										label="Hashing Algorithm"
										size="md"
									>
										<SelectOption value="Argon2id-s">Argon2id (standard)</SelectOption>
										<SelectOption value="Argon2id-h">Argon2id (hardened)</SelectOption>
										<SelectOption value="Argon2id-p">Argon2id (paranoid)</SelectOption>
										<SelectOption value="BalloonBlake3-s">BLAKE3-Balloon (standard)</SelectOption>
										<SelectOption value="BalloonBlake3-h">BLAKE3-Balloon (hardened)</SelectOption>
										<SelectOption value="BalloonBlake3-p">BLAKE3-Balloon (paranoid)</SelectOption>
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
