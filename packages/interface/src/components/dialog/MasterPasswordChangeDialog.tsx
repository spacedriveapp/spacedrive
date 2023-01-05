import { Algorithm, useLibraryMutation } from '@sd/client';
import { Button, Dialog, Input, Select, SelectOption } from '@sd/ui';
import { writeText } from '@tauri-apps/api/clipboard';
import { zxcvbn, zxcvbnOptions } from '@zxcvbn-ts/core';
import zxcvbnCommonPackage from '@zxcvbn-ts/language-common';
import zxcvbnEnPackage from '@zxcvbn-ts/language-en';
import clsx from 'clsx';
import cryptoRandomString from 'crypto-random-string';
import { ArrowsClockwise, Clipboard, Eye, EyeSlash } from 'phosphor-react';
import { ReactNode, useState } from 'react';
import { SubmitHandler, useForm } from 'react-hook-form';

import { getHashingAlgorithmSettings } from '../../screens/settings/library/KeysSetting';
import { generatePassword } from '../key/KeyMounter';
import { GenericAlertDialogProps } from './AlertDialog';

export interface MasterPasswordChangeDialogProps {
	trigger: ReactNode;
	setAlertDialogData: (data: GenericAlertDialogProps) => void;
}
type FormValues = {
	masterPassword: string;
	masterPassword2: string;
	secretKey: string | null;
};

export const MasterPasswordChangeDialog = (props: MasterPasswordChangeDialogProps) => {
	const { trigger } = props;

	const form = useForm<FormValues>({
		defaultValues: {
			masterPassword: '',
			masterPassword2: '',
			secretKey: ''
		}
	});

	const [encryptionAlgo, setEncryptionAlgo] = useState('XChaCha20Poly1305');
	const [hashingAlgo, setHashingAlgo] = useState('Argon2id-s');
	const [showMasterPasswordDialog, setShowMasterPasswordDialog] = useState(false);
	const changeMasterPassword = useLibraryMutation('keys.changeMasterPassword');
	const [showMasterPassword1, setShowMasterPassword1] = useState(false);
	const [showMasterPassword2, setShowMasterPassword2] = useState(false);
	const [showSecretKey, setShowSecretKey] = useState(false);

	const MP1CurrentEyeIcon = showMasterPassword1 ? EyeSlash : Eye;
	const MP2CurrentEyeIcon = showMasterPassword2 ? EyeSlash : Eye;
	const SKCurrentEyeIcon = showSecretKey ? EyeSlash : Eye;

	const onSubmit: SubmitHandler<FormValues> = (data) => {
		if (data.masterPassword !== data.masterPassword2) {
			props.setAlertDialogData({
				open: true,
				title: 'Error',
				description: '',
				value: 'Passwords are not the same, please try again.',
				inputBox: false
			});
		} else {
			const hashing_algorithm = getHashingAlgorithmSettings(hashingAlgo);
			const sk = data.secretKey !== null ? data.secretKey : null;

			changeMasterPassword.mutate(
				{
					algorithm: encryptionAlgo as Algorithm,
					hashing_algorithm,
					password: data.masterPassword,
					secret_key: sk
				},
				{
					onSuccess: () => {
						setShowMasterPasswordDialog(false);
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
						setShowMasterPasswordDialog(false);
						props.setAlertDialogData({
							open: true,
							title: 'Master Password Change Error',
							description: '',
							value: 'There was an error while changing your master password.',
							inputBox: false
						});
					}
				}
			);

			form.reset();
		}
	};

	return (
		<>
			<form onSubmit={form.handleSubmit(onSubmit)}>
				<Dialog
					open={showMasterPasswordDialog}
					setOpen={setShowMasterPasswordDialog}
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
							type={showMasterPassword1 ? 'text' : 'password'}
						/>
						<Button
							onClick={() => {
								const password = generatePassword(32);
								form.setValue('masterPassword', password);
								form.setValue('masterPassword2', password);
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
								writeText(form.watch('masterPassword') as string);
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
					<div className="relative flex flex-grow mb-2">
						<Input
							className={`flex-grow !py-0.5}`}
							placeholder="New password (again)"
							required
							{...form.register('masterPassword2', { required: true })}
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

					<div className="relative flex flex-grow mb-2">
						<Input
							className={`flex-grow !py-0.5}`}
							placeholder="Key secret"
							{...form.register('secretKey', { required: false })}
							type={showSecretKey ? 'text' : 'password'}
						/>
						<Button
							// onClick={() => setShowMasterPassword2(!showMasterPassword2)}
							onClick={() => {
								form.setValue('secretKey', cryptoRandomString({ length: 24 }));
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
								writeText(form.watch('secretKey') as string);
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

					<PasswordMeter password={form.watch('masterPassword')} />

					<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
						<div className="flex flex-col">
							<span className="text-xs font-bold">Encryption</span>
							<Select
								className="mt-2"
								value={encryptionAlgo}
								onChange={(e) => setEncryptionAlgo(e)}
							>
								<SelectOption value="XChaCha20Poly1305">XChaCha20-Poly1305</SelectOption>
								<SelectOption value="Aes256Gcm">AES-256-GCM</SelectOption>
							</Select>
						</div>
						<div className="flex flex-col">
							<span className="text-xs font-bold">Hashing</span>
							<Select className="mt-2" value={hashingAlgo} onChange={(e) => setHashingAlgo(e)}>
								<SelectOption value="Argon2id-s">Argon2id (standard)</SelectOption>
								<SelectOption value="Argon2id-h">Argon2id (hardened)</SelectOption>
								<SelectOption value="Argon2id-p">Argon2id (paranoid)</SelectOption>
								<SelectOption value="BalloonBlake3-s">Blake3-Balloon (standard)</SelectOption>
								<SelectOption value="BalloonBlake3-h">Blake3-Balloon (hardened)</SelectOption>
								<SelectOption value="BalloonBlake3-p">Blake3-Balloon (paranoid)</SelectOption>
							</Select>
						</div>
					</div>
				</Dialog>
			</form>
		</>
	);
};

export const PasswordMeter = (props: { password: string }) => {
	const ratings = ['Poor', 'Weak', 'Good', 'Strong', 'Perfect'];

	const options = {
		dictionary: {
			...zxcvbnCommonPackage.dictionary,
			...zxcvbnEnPackage.dictionary
		},
		graps: zxcvbnCommonPackage.adjacencyGraphs,
		translations: zxcvbnEnPackage.translations
	};
	zxcvbnOptions.setOptions(options);
	const zx = zxcvbn(props.password);

	const innerDiv = {
		width: `${zx.score !== 0 ? zx.score * 25 : 12.5}%`,
		height: '5px',
		borderRadius: 80
	};

	return (
		<div className="mt-4 mb-5 relative flex flex-grow">
			<div className="mt-2 w-4/5 h-[5px] rounded-[80px]">
				<div
					style={innerDiv}
					className={clsx(
						zx.score === 0 && 'bg-red-700',
						zx.score === 1 && 'bg-red-500',
						zx.score === 2 && 'bg-amber-400',
						zx.score === 3 && 'bg-lime-500',
						zx.score === 4 && 'bg-accent'
					)}
				/>
			</div>
			<span
				className={clsx(
					'absolute font-[750] right-[5px] text-sm pr-1 pl-1',
					zx.score === 0 && 'text-red-700',
					zx.score === 1 && 'text-red-500',
					zx.score === 2 && 'text-amber-400',
					zx.score === 3 && 'text-lime-500',
					zx.score === 4 && 'text-accent'
				)}
			>
				{ratings[zx.score]}
			</span>
		</div>
	);
};
