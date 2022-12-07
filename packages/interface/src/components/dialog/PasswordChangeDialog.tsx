import { useLibraryMutation } from '@sd/client';
import { Button, Dialog, Input, Select, SelectOption } from '@sd/ui';
import { zxcvbn, zxcvbnOptions } from '@zxcvbn-ts/core';
import zxcvbnCommonPackage from '@zxcvbn-ts/language-common';
import zxcvbnEnPackage from '@zxcvbn-ts/language-en';
import clsx from 'clsx';
import { Eye, EyeSlash } from 'phosphor-react';
import { ReactNode, useState } from 'react';
import { SubmitHandler, useForm } from 'react-hook-form';

import { getCryptoSettings } from '../../screens/settings/library/KeysSetting';

export const PasswordChangeDialog = (props: { trigger: ReactNode }) => {
	type FormValues = {
		masterPassword: string;
		masterPassword2: string;
		encryptionAlgo: string;
		hashingAlgo: string;
	};

	const [secretKey, setSecretKey] = useState('');

	const { register, handleSubmit, getValues, setValue } = useForm<FormValues>({
		defaultValues: {
			masterPassword: '',
			masterPassword2: '',
			encryptionAlgo: 'XChaCha20Poly1305',
			hashingAlgo: 'Argon2id-s'
		}
	});

	const onSubmit: SubmitHandler<FormValues> = (data) => {
		if (data.masterPassword !== data.masterPassword2) {
			alert('Passwords are not the same.');
		} else {
			const [algorithm, hashing_algorithm] = getCryptoSettings(
				data.encryptionAlgo,
				data.hashingAlgo
			);

			changeMasterPassword.mutate(
				{ algorithm, hashing_algorithm, password: data.masterPassword },
				{
					onSuccess: (sk) => {
						setSecretKey(sk);

						setShowMasterPasswordDialog(false);
						setShowSecretKeyDialog(true);
					},
					onError: () => {
						// this should never really happen
						alert('There was an error while changing your master password.');
					}
				}
			);
		}
	};

	const [passwordMeterMasterPw, setPasswordMeterMasterPw] = useState(''); // this is needed as the password meter won't update purely with react-hook-for
	const [showMasterPasswordDialog, setShowMasterPasswordDialog] = useState(false);
	const [showSecretKeyDialog, setShowSecretKeyDialog] = useState(false);
	const changeMasterPassword = useLibraryMutation('keys.changeMasterPassword');
	const [showMasterPassword1, setShowMasterPassword1] = useState(false);
	const [showMasterPassword2, setShowMasterPassword2] = useState(false);
	const MP1CurrentEyeIcon = showMasterPassword1 ? EyeSlash : Eye;
	const MP2CurrentEyeIcon = showMasterPassword2 ? EyeSlash : Eye;
	const { trigger } = props;

	return (
		<>
			<form onSubmit={handleSubmit(onSubmit)}>
				<Dialog
					open={showMasterPasswordDialog}
					setOpen={setShowMasterPasswordDialog}
					title="Change Master Password"
					description="Select a new master password for your key manager."
					ctaDanger={true}
					loading={changeMasterPassword.isLoading}
					ctaLabel="Change"
					trigger={trigger}
				>
					<div className="relative flex flex-grow mt-3 mb-2">
						<Input
							className={`flex-grow w-max !py-0.5`}
							placeholder="New Password"
							required
							{...register('masterPassword', { required: true })}
							onChange={(e) => setPasswordMeterMasterPw(e.target.value)}
							value={passwordMeterMasterPw}
							type={showMasterPassword1 ? 'text' : 'password'}
						/>
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
							placeholder="New Password (again)"
							required
							{...register('masterPassword2', { required: true })}
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

					<PasswordMeter password={passwordMeterMasterPw} />

					<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
						<div className="flex flex-col">
							<span className="text-xs font-bold">Encryption</span>
							<Select
								className="mt-2"
								value={getValues('encryptionAlgo')}
								onChange={(e) => setValue('encryptionAlgo', e)}
							>
								<SelectOption value="XChaCha20Poly1305">XChaCha20-Poly1305</SelectOption>
								<SelectOption value="Aes256Gcm">AES-256-GCM</SelectOption>
							</Select>
						</div>
						<div className="flex flex-col">
							<span className="text-xs font-bold">Hashing</span>
							<Select
								className="mt-2"
								value={getValues('hashingAlgo')}
								onChange={(e) => setValue('hashingAlgo', e)}
							>
								<SelectOption value="Argon2id-s">Argon2id (standard)</SelectOption>
								<SelectOption value="Argon2id-h">Argon2id (hardened)</SelectOption>
								<SelectOption value="Argon2id-p">Argon2id (paranoid)</SelectOption>
							</Select>
						</div>
					</div>
				</Dialog>
			</form>
			<Dialog
				open={showSecretKeyDialog}
				setOpen={setShowSecretKeyDialog}
				title="Secret Key"
				description="Please store this secret key securely as it is needed to access your key manager."
				ctaAction={() => {
					setShowSecretKeyDialog(false);
				}}
				ctaLabel="Done"
				trigger={<></>}
			>
				<Input
					className="flex-grow w-full mt-3"
					value={secretKey}
					placeholder="Secret Key"
					disabled={true}
				/>
			</Dialog>
		</>
	);
};

const PasswordMeter = (props: { password: string }) => {
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
