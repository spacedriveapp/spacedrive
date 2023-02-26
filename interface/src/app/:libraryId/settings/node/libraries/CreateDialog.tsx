import { useQueryClient } from '@tanstack/react-query';
import { ArrowsClockwise, Clipboard, Eye, EyeSlash } from 'phosphor-react';
import { useState } from 'react';
import {
	Algorithm,
	HASHING_ALGOS,
	HashingAlgoSlug,
	generatePassword,
	hashingAlgoSlugSchema,
	useBridgeMutation
} from '@sd/client';
import {
	Button,
	Dialog,
	PasswordMeter,
	Select,
	SelectOption,
	UseDialogProps,
	useDialog
} from '@sd/ui';
import { forms } from '@sd/ui';

const { Input, z, useZodForm } = forms;

const schema = z.object({
	name: z.string(),
	password: z.string(),
	password_validate: z.string(),
	algorithm: z.string(),
	hashing_algorithm: hashingAlgoSlugSchema
});

export default (props: UseDialogProps) => {
	const dialog = useDialog(props);

	const form = useZodForm({
		schema,
		defaultValues: {
			password: '',
			algorithm: 'XChaCha20Poly1305',
			hashing_algorithm: 'Argon2id-s'
		}
	});

	const [showMasterPassword1, setShowMasterPassword1] = useState(false);
	const [showMasterPassword2, setShowMasterPassword2] = useState(false);
	const MP1CurrentEyeIcon = showMasterPassword1 ? EyeSlash : Eye;
	const MP2CurrentEyeIcon = showMasterPassword2 ? EyeSlash : Eye;

	const queryClient = useQueryClient();
	const createLibrary = useBridgeMutation('library.create', {
		onSuccess: (library) => {
			queryClient.setQueryData(['library.list'], (libraries: any) => [
				...(libraries || []),
				library
			]);
		},
		onError: (err: any) => {
			console.error(err);
		}
	});

	const onSubmit = form.handleSubmit(async (data) => {
		if (data.password !== data.password_validate) {
			alert('Passwords are not the same');
		} else {
			await createLibrary.mutateAsync({
				...data,
				algorithm: data.algorithm as Algorithm,
				hashing_algorithm: HASHING_ALGOS[data.hashing_algorithm],
				auth: {
					type: 'Password',
					value: data.password
				}
			});
		}
	});

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			dialog={dialog}
			title="Create New Library"
			description="Choose a name for your new library, you can configure this and more settings from the library settings later on."
			submitDisabled={!form.formState.isValid}
			ctaLabel="Create"
		>
			<div className="relative flex flex-col">
				<p className="my-2 text-sm font-bold">Library name</p>
				<Input
					className="w-full grow"
					placeholder="My Cool Library"
					{...form.register('name', { required: true })}
				/>
			</div>

			{/* TODO: Proper UI for this. Maybe checkbox for encrypted or not and then reveal these fields. Select encrypted by default. */}
			{/* <span className="text-sm">Make the secret key field empty to skip key setup.</span> */}

			<div className="relative flex flex-col">
				<p className="mt-2 mb-1 text-center text-[0.95rem] font-bold">Key Manager</p>
				<div className="my-1 h-[2px] w-full bg-gray-500" />

				<p className="my-2 text-sm font-bold">Master password</p>
				<div className="relative mb-2 flex grow">
					<Input
						className="grow !py-0.5"
						placeholder="Password"
						type={showMasterPassword1 ? 'text' : 'password'}
						{...form.register('password')}
					/>
					<Button
						onClick={() => {
							const password = generatePassword(32);

							form.setValue('password', password);
							form.setValue('password_validate', password);

							setShowMasterPassword1(true);
							setShowMasterPassword2(true);
						}}
						size="icon"
						className="absolute right-[65px] top-[5px] border-none"
					>
						<ArrowsClockwise className="h-4 w-4" />
					</Button>
					<Button
						onClick={() => {
							navigator.clipboard.writeText(form.watch('password') as string);
						}}
						size="icon"
						className="absolute right-[35px] top-[5px] border-none"
					>
						<Clipboard className="h-4 w-4" />
					</Button>
					<Button
						onClick={() => setShowMasterPassword1(!showMasterPassword1)}
						size="icon"
						className="absolute right-[5px] top-[5px] border-none"
					>
						<MP1CurrentEyeIcon className="h-4 w-4" />
					</Button>
				</div>
			</div>
			<div className="relative flex flex-col">
				<p className="my-2 text-sm font-bold">Master password (again)</p>
				<div className="relative mb-2 flex grow">
					<Input
						className="grow !py-0.5"
						placeholder="Password"
						type={showMasterPassword2 ? 'text' : 'password'}
						{...form.register('password_validate')}
					/>
					<Button
						onClick={() => setShowMasterPassword2(!showMasterPassword2)}
						size="icon"
						className="absolute right-[5px] top-[5px] border-none"
					>
						<MP2CurrentEyeIcon className="h-4 w-4" />
					</Button>
				</div>
			</div>

			<div className="mt-4 mb-3 grid w-full grid-cols-2 gap-4">
				<div className="flex flex-col">
					<span className="text-sm font-bold">Encryption</span>
					<Select
						className="mt-2"
						value={form.watch('algorithm')}
						onChange={(e) => form.setValue('algorithm', e)}
					>
						<SelectOption value="XChaCha20Poly1305">XChaCha20-Poly1305</SelectOption>
						<SelectOption value="Aes256Gcm">AES-256-GCM</SelectOption>
					</Select>
				</div>
				<div className="flex flex-col">
					<span className="text-sm font-bold">Hashing</span>
					<Select
						className="mt-2"
						value={form.watch('hashing_algorithm')}
						onChange={(e) => form.setValue('hashing_algorithm', e as HashingAlgoSlug)}
					>
						<SelectOption value="Argon2id-s">Argon2id (standard)</SelectOption>
						<SelectOption value="Argon2id-h">Argon2id (hardened)</SelectOption>
						<SelectOption value="Argon2id-p">Argon2id (paranoid)</SelectOption>
						<SelectOption value="BalloonBlake3-s">BLAKE3-Balloon (standard)</SelectOption>
						<SelectOption value="BalloonBlake3-h">BLAKE3-Balloon (hardened)</SelectOption>
						<SelectOption value="BalloonBlake3-p">BLAKE3-Balloon (paranoid)</SelectOption>
					</Select>
				</div>
			</div>

			<PasswordMeter password={form.watch('password')} />
		</Dialog>
	);
};
