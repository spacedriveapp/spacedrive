import { ArrowsClockwise, Clipboard, Eye, EyeSlash } from 'phosphor-react';
import { useState } from 'react';
import { Algorithm, useLibraryMutation } from '@sd/client';
import {
	Button,
	Dialog,
	Input,
	PasswordMeter,
	Select,
	SelectOption,
	UseDialogProps,
	useDialog
} from '@sd/ui';
import { useZodForm, z } from '@sd/ui/src/forms';
import {
	HashingAlgoSlug,
	getHashingAlgorithmSettings
} from '~/app/:libraryId/settings/library/keys';
import { showAlertDialog } from '~/components/AlertDialog';
import { generatePassword } from '~/components/KeyManager/Mounter';

const schema = z.object({
	masterPassword: z.string(),
	masterPassword2: z.string(),
	encryptionAlgo: z.string(),
	hashingAlgo: z.string()
});

export default (props: UseDialogProps) => {
	const changeMasterPassword = useLibraryMutation('keys.changeMasterPassword', {
		onSuccess: () => {
			showAlertDialog({
				title: 'Success',
				value: 'Your master password was changed successfully'
			});
		},
		onError: () => {
			// this should never really happen
			showAlertDialog({
				title: 'Master Password Change Error',
				value: 'There was an error while changing your master password.'
			});
		}
	});

	const [show, setShow] = useState({
		masterPassword: false,
		masterPassword2: false
	});

	const dialog = useDialog(props);

	const MP1CurrentEyeIcon = show.masterPassword ? EyeSlash : Eye;
	const MP2CurrentEyeIcon = show.masterPassword2 ? EyeSlash : Eye;

	const form = useZodForm({
		schema,
		defaultValues: {
			encryptionAlgo: 'XChaCha20Poly1305',
			hashingAlgo: 'Argon2id-s',
			masterPassword: '',
			masterPassword2: ''
		}
	});

	const onSubmit = form.handleSubmit((data) => {
		if (data.masterPassword !== data.masterPassword2) {
			showAlertDialog({
				title: 'Error',
				value: 'Passwords are not the same, please try again.'
			});
		} else {
			const hashing_algorithm = getHashingAlgorithmSettings(data.hashingAlgo as HashingAlgoSlug);
			return changeMasterPassword.mutateAsync({
				algorithm: data.encryptionAlgo as Algorithm,
				hashing_algorithm,
				password: data.masterPassword
			});
		}
	});

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			dialog={dialog}
			title="Change Master Password"
			description="Select a new master password for your key manager."
			ctaDanger={true}
			ctaLabel="Change"
		>
			<div className="relative mt-3 mb-2 flex grow">
				<Input
					className={`w-max grow !py-0.5`}
					placeholder="New password"
					type={show.masterPassword ? 'text' : 'password'}
					{...form.register('masterPassword', { required: true })}
				/>
				<Button
					onClick={() => {
						const password = generatePassword(32);
						form.setValue('masterPassword', password);
						form.setValue('masterPassword2', password);
						setShow((old) => ({
							...old,
							masterPassword: true,
							masterPassword2: true
						}));
					}}
					size="icon"
					className="absolute right-[65px] top-[5px] border-none"
					type="button"
				>
					<ArrowsClockwise className="h-4 w-4" />
				</Button>
				<Button
					type="button"
					onClick={() => {
						navigator.clipboard.writeText(form.watch('masterPassword') as string);
					}}
					size="icon"
					className="absolute right-[35px] top-[5px] border-none"
				>
					<Clipboard className="h-4 w-4" />
				</Button>
				<Button
					onClick={() => setShow((old) => ({ ...old, masterPassword: !old.masterPassword }))}
					size="icon"
					className="absolute right-[5px] top-[5px] border-none"
					type="button"
				>
					<MP1CurrentEyeIcon className="h-4 w-4" />
				</Button>
			</div>
			<div className="relative mb-2 flex grow">
				<Input
					className={`!py-0.5} grow`}
					placeholder="New password (again)"
					type={show.masterPassword2 ? 'text' : 'password'}
					{...form.register('masterPassword2', { required: true })}
				/>
				<Button
					onClick={() => setShow((old) => ({ ...old, masterPassword2: !old.masterPassword2 }))}
					size="icon"
					className="absolute right-[5px] top-[5px] border-none"
					type="button"
				>
					<MP2CurrentEyeIcon className="h-4 w-4" />
				</Button>
			</div>

			<PasswordMeter password={form.watch('masterPassword')} />

			<div className="mt-4 mb-3 grid w-full grid-cols-2 gap-4">
				<div className="flex flex-col">
					<span className="text-xs font-bold">Encryption</span>
					<Select
						className="mt-2"
						value={form.watch('encryptionAlgo')}
						onChange={(e) => form.setValue('encryptionAlgo', e)}
					>
						<SelectOption value="XChaCha20Poly1305">XChaCha20-Poly1305</SelectOption>
						<SelectOption value="Aes256Gcm">AES-256-GCM</SelectOption>
					</Select>
				</div>
				<div className="flex flex-col">
					<span className="text-xs font-bold">Hashing</span>
					<Select
						className="mt-2"
						value={form.watch('hashingAlgo')}
						onChange={(e) => form.setValue('hashingAlgo', e)}
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
		</Dialog>
	);
};
