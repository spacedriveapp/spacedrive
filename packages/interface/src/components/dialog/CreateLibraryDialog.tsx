import { Algorithm, useBridgeMutation } from '@sd/client';
import { Button, Dialog, Input, Select, SelectOption } from '@sd/ui';
import { useQueryClient } from '@tanstack/react-query';
import { writeText } from '@tauri-apps/api/clipboard';
import cryptoRandomString from 'crypto-random-string';
import { ArrowsClockwise, Clipboard, Eye, EyeSlash } from 'phosphor-react';
import { PropsWithChildren, useState } from 'react';
import { useForm } from 'react-hook-form';

import { getHashingAlgorithmSettings } from '../../screens/settings/library/KeysSetting';
import { generatePassword } from '../key/KeyMounter';
import { PasswordMeter } from '../key/PasswordMeter';

export default function CreateLibraryDialog({
	children,
	onSubmit,
	open,
	setOpen
}: PropsWithChildren<{ onSubmit?: () => void; open: boolean; setOpen: (state: boolean) => void }>) {
	const queryClient = useQueryClient();

	const form = useForm({
		defaultValues: {
			name: '',
			password: '' as string,
			password_validate: '' as string,
			secret_key: '' as string | null,
			algorithm: 'XChaCha20Poly1305',
			hashing_algorithm: 'Argon2id-s'
		}
	});

	const [showMasterPassword1, setShowMasterPassword1] = useState(false);
	const [showMasterPassword2, setShowMasterPassword2] = useState(false);
	const [showSecretKey, setShowSecretKey] = useState(false);
	const MP1CurrentEyeIcon = showMasterPassword1 ? EyeSlash : Eye;
	const MP2CurrentEyeIcon = showMasterPassword2 ? EyeSlash : Eye;
	const SKCurrentEyeIcon = showSecretKey ? EyeSlash : Eye;

	const createLibrary = useBridgeMutation('library.create', {
		onSuccess: (library) => {
			queryClient.setQueryData(['library.list'], (libraries: any) => [
				...(libraries || []),
				library
			]);

			if (onSubmit) onSubmit();
			setOpen(false);
			form.reset();
		},
		onError: (err: any) => {
			console.error(err);
		}
	});
	const doSubmit = form.handleSubmit((data) => {
		if (data.secret_key === '') {
			data.secret_key = null;
		}

		if (data.password !== data.password_validate) {
			alert('Passwords are not the same');
		} else {
			return createLibrary.mutateAsync({
				...data,
				algorithm: data.algorithm as Algorithm,
				hashing_algorithm: getHashingAlgorithmSettings(data.hashing_algorithm)
			});
		}
	});

	return (
		<Dialog
			open={open}
			setOpen={setOpen}
			title="Create New Library"
			description="Choose a name for your new library, you can configure this and more settings from the library settings later on."
			ctaAction={doSubmit}
			loading={form.formState.isSubmitting}
			submitDisabled={!form.formState.isValid}
			ctaLabel="Create"
			trigger={children}
		>
			<form onSubmit={doSubmit}>
				<div className="relative flex flex-col">
					<p className="text-sm mt-2 mb-2 font-bold">Library name</p>
					<Input
						className="flex-grow w-full"
						placeholder="My Cool Library"
						disabled={form.formState.isSubmitting}
						{...form.register('name', { required: true })}
					/>
				</div>

				{/* TODO: Proper UI for this. Maybe checkbox for encrypted or not and then reveal these fields. Select encrypted by default. */}
				{/* <span className="text-sm">Make the secret key field empty to skip key setup.</span> */}

				<div className="relative flex flex-col">
					<p className="text-center mt-2 mb-1 text-[0.95rem] font-bold">Key Manager</p>
					<div className="w-full my-1 h-[2px] bg-gray-500" />

					<p className="text-sm mt-2 mb-2 font-bold">Master password</p>
					<div className="relative flex flex-grow mb-2">
						<Input
							className="flex-grow !py-0.5"
							disabled={form.formState.isSubmitting}
							{...form.register('password')}
							placeholder="Password"
							type={showMasterPassword1 ? 'text' : 'password'}
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
							className="border-none absolute right-[65px] top-[5px]"
							type="button"
						>
							<ArrowsClockwise className="w-4 h-4" />
						</Button>
						<Button
							type="button"
							onClick={() => {
								writeText(form.watch('password') as string);
							}}
							size="icon"
							className="border-none absolute right-[35px] top-[5px]"
						>
							<Clipboard className="w-4 h-4" />
						</Button>
						<Button
							onClick={() => setShowMasterPassword1(!showMasterPassword1)}
							size="icon"
							className="border-none absolute right-[5px] top-[5px]"
							type="button"
						>
							<MP1CurrentEyeIcon className="w-4 h-4" />
						</Button>
					</div>
				</div>
				<div className="relative flex flex-col">
					<p className="text-sm mt-2 mb-2 font-bold">Master password (again)</p>
					<div className="relative flex flex-grow mb-2">
						<Input
							className="flex-grow !py-0.5"
							disabled={form.formState.isSubmitting}
							{...form.register('password_validate')}
							placeholder="Password"
							type={showMasterPassword2 ? 'text' : 'password'}
						/>
						<Button
							onClick={() => setShowMasterPassword2(!showMasterPassword2)}
							size="icon"
							className="border-none absolute right-[5px] top-[5px]"
							type="button"
						>
							<MP2CurrentEyeIcon className="w-4 h-4" />
						</Button>
					</div>
				</div>
				<div className="relative flex flex-col">
					<p className="text-sm mt-2 mb-2 font-bold">Key secret (optional)</p>
					<div className="relative flex flex-grow mb-2">
						<Input
							className="flex-grow !py-0.5"
							placeholder="Secret"
							disabled={form.formState.isSubmitting}
							{...form.register('secret_key')}
							type={showSecretKey ? 'text' : 'password'}
						/>
						<Button
							onClick={() => {
								form.setValue('secret_key', cryptoRandomString({ length: 24 }));
								setShowSecretKey(true);
							}}
							size="icon"
							className="border-none absolute right-[65px] top-[5px]"
							type="button"
						>
							<ArrowsClockwise className="w-4 h-4" />
						</Button>
						<Button
							type="button"
							onClick={() => {
								writeText(form.watch('secret_key') as string);
							}}
							size="icon"
							className="border-none absolute right-[35px] top-[5px]"
						>
							<Clipboard className="w-4 h-4" />
						</Button>
						<Button
							onClick={() => setShowSecretKey(!showSecretKey)}
							size="icon"
							className="border-none absolute right-[5px] top-[5px]"
							type="button"
						>
							<SKCurrentEyeIcon className="w-4 h-4" />
						</Button>
					</div>
				</div>

				<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
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
							onChange={(e) => form.setValue('hashing_algorithm', e)}
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
			</form>
		</Dialog>
	);
}
