import { Algorithm, useLibraryMutation } from '@sd/client';
import { Button, Dialog, Input, Select, SelectOption } from '@sd/ui';
import cryptoRandomString from 'crypto-random-string';
import { ArrowsClockwise, Clipboard, Eye, EyeSlash } from 'phosphor-react';
import { ReactNode, useState } from 'react';
import { useForm } from 'react-hook-form';

import { getHashingAlgorithmSettings } from '../../screens/settings/library/KeysSetting';
import { generatePassword } from '../key/KeyMounter';
import { PasswordMeter } from '../key/PasswordMeter';
import { GenericAlertDialogProps } from './AlertDialog';

export interface MasterPasswordChangeDialogProps {
	trigger: ReactNode;
	setAlertDialogData: (data: GenericAlertDialogProps) => void;
}

type FormValues = {
	masterPassword: string;
	masterPassword2: string;
	secretKey: string | null;
	encryptionAlgo: string;
	hashingAlgo: string;
};

export const MasterPasswordChangeDialog = (props: MasterPasswordChangeDialogProps) => {
	const { trigger } = props;

	const form = useForm<FormValues>({
		defaultValues: {
			masterPassword: '',
			masterPassword2: '',
			secretKey: '',
			encryptionAlgo: 'XChaCha20Poly1305',
			hashingAlgo: 'Argon2id-s'
		}
	});

	const changeMasterPassword = useLibraryMutation('keys.changeMasterPassword', {
		onSuccess: () => {
			setShow((old) => ({ ...old, masterPasswordDialog: false }));
			props.setAlertDialogData({
				open: true,
				title: 'Success',
				description: '',
				value: 'Your master password was changed successfully',
				inputBox: false
			});
		},
		onError: () => {
			// this should never really happen
			setShow((old) => ({ ...old, masterPasswordDialog: false }));
			props.setAlertDialogData({
				open: true,
				title: 'Master Password Change Error',
				description: '',
				value: 'There was an error while changing your master password.',
				inputBox: false
			});
		}
	});

	const [show, setShow] = useState({
		masterPasswordDialog: false,
		masterPassword: false,
		masterPassword2: false,
		secretKey: false
	});

	const MP1CurrentEyeIcon = show.masterPassword ? EyeSlash : Eye;
	const MP2CurrentEyeIcon = show.masterPassword2 ? EyeSlash : Eye;
	const SKCurrentEyeIcon = show.secretKey ? EyeSlash : Eye;

	const onSubmit = form.handleSubmit((data) => {
		if (data.masterPassword !== data.masterPassword2) {
			props.setAlertDialogData({
				open: true,
				title: 'Error',
				description: '',
				value: 'Passwords are not the same, please try again.',
				inputBox: false
			});
		} else {
			const hashing_algorithm = getHashingAlgorithmSettings(data.hashingAlgo);
			const sk = data.secretKey || null;

			changeMasterPassword.mutate({
				algorithm: data.encryptionAlgo as Algorithm,
				hashing_algorithm,
				password: data.masterPassword,
				secret_key: sk
			});

			form.reset();
		}
	});

	return (
		<>
			<form onSubmit={onSubmit}>
				<Dialog
					open={show.masterPasswordDialog}
					setOpen={(e) => {
						setShow((old) => ({ ...old, masterPasswordDialog: e }));
					}}
					title="Change Master Password"
					description="Select a new master password for your key manager. Leave the key secret blank to disable it."
					ctaDanger={true}
					loading={changeMasterPassword.isLoading}
					ctaLabel="Change"
					trigger={trigger}
				>
					<div className="relative flex flex-grow mt-3 mb-2">
						<Input
							className={`flex-grow w-max !py-0.5`}
							placeholder="New password"
							required
							{...form.register('masterPassword', { required: true })}
							type={show.masterPassword ? 'text' : 'password'}
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
							className="border-none absolute right-[65px] top-[5px]"
							type="button"
						>
							<ArrowsClockwise className="w-4 h-4" />
						</Button>
						<Button
							type="button"
							onClick={() => {
								navigator.clipboard.writeText(form.watch('masterPassword') as string);
							}}
							size="icon"
							className="border-none absolute right-[35px] top-[5px]"
						>
							<Clipboard className="w-4 h-4" />
						</Button>
						<Button
							onClick={() => setShow((old) => ({ ...old, masterPassword: !old.masterPassword }))}
							size="icon"
							className="border-none absolute right-[5px] top-[5px]"
							type="button"
						>
							<MP1CurrentEyeIcon className="w-4 h-4" />
						</Button>
					</div>
					<div className="relative flex flex-grow mb-2">
						<Input
							className={`flex-grow !py-0.5}`}
							placeholder="New password (again)"
							required
							{...form.register('masterPassword2', { required: true })}
							type={show.masterPassword2 ? 'text' : 'password'}
						/>
						<Button
							onClick={() => setShow((old) => ({ ...old, masterPassword2: !old.masterPassword2 }))}
							size="icon"
							className="border-none absolute right-[5px] top-[5px]"
							type="button"
						>
							<MP2CurrentEyeIcon className="w-4 h-4" />
						</Button>
					</div>

					<div className="relative flex flex-grow mb-2">
						<Input
							className={`flex-grow !py-0.5}`}
							placeholder="Key secret"
							{...form.register('secretKey', { required: false })}
							type={show.secretKey ? 'text' : 'password'}
						/>
						<Button
							// onClick={() => setmasterPassword2(!masterPassword2)}
							onClick={() => {
								form.setValue('secretKey', cryptoRandomString({ length: 24 }));
								setShow((old) => ({ ...old, secretKey: true }));
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
								navigator.clipboard.writeText(form.watch('secretKey') as string);
							}}
							size="icon"
							className="border-none absolute right-[35px] top-[5px]"
						>
							<Clipboard className="w-4 h-4" />
						</Button>
						<Button
							onClick={() => setShow((old) => ({ ...old, secretKey: !old.secretKey }))}
							size="icon"
							className="border-none absolute right-[5px] top-[5px]"
							type="button"
						>
							<SKCurrentEyeIcon className="w-4 h-4" />
						</Button>
					</div>

					<PasswordMeter password={form.watch('masterPassword')} />

					<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
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
			</form>
		</>
	);
};
