import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import {
	Algorithm,
	HashingAlgorithm,
	Params,
	useLibraryMutation,
	useLibraryQuery
} from '@sd/client';
import { Button, Input } from '@sd/ui';
import { save } from '@tauri-apps/api/dialog';
import clsx from 'clsx';
import { Eye, EyeSlash, Lock, Plus } from 'phosphor-react';
import { PropsWithChildren, useState } from 'react';
import { animated, useTransition } from 'react-spring';

import { AlertDialog } from '../../../components/dialog/AlertDialog';
import { BackupRestoreDialog } from '../../../components/dialog/BackupRestoreDialog';
import { KeyViewerDialog } from '../../../components/dialog/KeyViewerDialog';
import { PasswordChangeDialog } from '../../../components/dialog/PasswordChangeDialog';
import { ListOfKeys } from '../../../components/key/KeyList';
import { KeyMounter } from '../../../components/key/KeyMounter';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';
import { SettingsSubHeader } from '../../../components/settings/SettingsSubHeader';

interface Props extends DropdownMenu.MenuContentProps {
	trigger: React.ReactNode;
	transformOrigin?: string;
	disabled?: boolean;
}

export const KeyMounterDropdown = ({
	trigger,
	children,
	disabled,
	transformOrigin,
	className,
	...props
}: PropsWithChildren<Props>) => {
	const [open, setOpen] = useState(false);

	const transitions = useTransition(open, {
		from: {
			opacity: 0,
			transform: `scale(0.9)`,
			transformOrigin: transformOrigin || 'top'
		},
		enter: { opacity: 1, transform: 'scale(1)' },
		leave: { opacity: -0.5, transform: 'scale(0.95)' },
		config: { mass: 0.4, tension: 200, friction: 10 }
	});

	return (
		<DropdownMenu.Root open={open} onOpenChange={setOpen}>
			<DropdownMenu.Trigger>{trigger}</DropdownMenu.Trigger>
			{transitions(
				(styles, show) =>
					show && (
						<DropdownMenu.Portal forceMount>
							<DropdownMenu.Content forceMount asChild>
								<animated.div
									// most of this is copied over from the `OverlayPanel`
									className={clsx(
										'flex flex-col',
										'z-50 m-2 space-y-1',
										'select-none cursor-default rounded-lg',
										'text-left text-sm text-ink',
										'bg-app-overlay/80 backdrop-blur',
										// 'border border-app-overlay',
										'shadow-2xl shadow-black/60 ',
										className
									)}
									style={styles}
								>
									{children}
								</animated.div>
							</DropdownMenu.Content>
						</DropdownMenu.Portal>
					)
			)}
		</DropdownMenu.Root>
	);
};

export default function KeysSettings() {
	const hasMasterPw = useLibraryQuery(['keys.hasMasterPassword']);
	const setMasterPasswordMutation = useLibraryMutation('keys.setMasterPassword');
	const unmountAll = useLibraryMutation('keys.unmountAll');
	const clearMasterPassword = useLibraryMutation('keys.clearMasterPassword');
	const backupKeystore = useLibraryMutation('keys.backupKeystore');

	const [showMasterPassword, setShowMasterPassword] = useState(false);
	const [showSecretKey, setShowSecretKey] = useState(false);
	const [masterPassword, setMasterPassword] = useState('');
	const [secretKey, setSecretKey] = useState('');

	const [showAlertDialog, setShowAlertDialog] = useState(false);
	const [alertDialogData, setAlertDialogData] = useState({
		title: '',
		description: '',
		value: '',
		inputBox: false
	});

	const MPCurrentEyeIcon = showMasterPassword ? EyeSlash : Eye;
	const SKCurrentEyeIcon = showSecretKey ? EyeSlash : Eye;

	if (!hasMasterPw?.data) {
		return (
			<>
				<div className="p-2 mr-20 ml-20 mt-10">
					<div className="relative flex flex-grow mb-2">
						<Input
							value={masterPassword}
							onChange={(e) => setMasterPassword(e.target.value)}
							autoFocus
							type={showMasterPassword ? 'text' : 'password'}
							className="flex-grow !py-0.5"
							placeholder="Master Password"
						/>
						<Button
							onClick={() => setShowMasterPassword(!showMasterPassword)}
							size="icon"
							className="border-none absolute right-[5px] top-[5px]"
						>
							<MPCurrentEyeIcon className="w-4 h-4" />
						</Button>
					</div>

					<div className="relative flex flex-grow mb-2">
						<Input
							value={secretKey}
							onChange={(e) => setSecretKey(e.target.value)}
							type={showSecretKey ? 'text' : 'password'}
							className="flex-grow !py-0.5"
							placeholder="Secret Key"
						/>
						<Button
							onClick={() => setShowSecretKey(!showSecretKey)}
							size="icon"
							className="border-none absolute right-[5px] top-[5px]"
						>
							<SKCurrentEyeIcon className="w-4 h-4" />
						</Button>
					</div>

					<Button
						className="w-full"
						variant="accent"
						disabled={setMasterPasswordMutation.isLoading}
						onClick={() => {
							if (masterPassword !== '' && secretKey !== '') {
								setMasterPassword('');
								setSecretKey('');
								setMasterPasswordMutation.mutate(
									{ password: masterPassword, secret_key: secretKey },
									{
										onError: () => {
											setAlertDialogData({
												title: 'Unlock Error',
												description: '',
												value: 'The information provided to the key manager was incorrect',
												inputBox: false
											});

											setShowAlertDialog(true);
										}
									}
								);
							}
						}}
					>
						Unlock
					</Button>
				</div>
				<AlertDialog
					open={showAlertDialog}
					setOpen={setShowAlertDialog}
					title={alertDialogData.title}
					description={alertDialogData.description}
					value={alertDialogData.value}
					inputBox={alertDialogData.inputBox}
				/>
			</>
		);
	} else {
		return (
			<>
				<SettingsContainer>
					<SettingsHeader
						title="Keys"
						description="Manage your keys."
						rightArea={
							<div className="flex flex-row items-center">
								<Button
									size="icon"
									onClick={() => {
										unmountAll.mutate(null);
										clearMasterPassword.mutate(null);
									}}
									variant="outline"
									className="text-ink-faint"
								>
									<Lock className="w-4 h-4 text-ink-faint" />
								</Button>
								<KeyMounterDropdown
									trigger={
										<Button size="icon" variant="outline" className="text-ink-faint">
											<Plus className="w-4 h-4 text-ink-faint" />
										</Button>
									}
								>
									<KeyMounter />
								</KeyMounterDropdown>
							</div>
						}
					/>
					<div className="grid space-y-2">
						<ListOfKeys />
					</div>

					<SettingsSubHeader title="Password Options" />
					<div className="flex flex-row">
						<PasswordChangeDialog
							setShowDialog={setShowAlertDialog}
							setDialogData={setAlertDialogData}
							trigger={
								<Button size="sm" variant="gray" className="mr-2">
									Change Master Password
								</Button>
							}
						/>
						<KeyViewerDialog
							trigger={
								<Button size="sm" variant="gray" className="mr-2">
									View Key Values
								</Button>
							}
						/>
					</div>

					<SettingsSubHeader title="Data Recovery" />
					<div className="flex flex-row">
						<Button
							size="sm"
							variant="gray"
							className="mr-2"
							type="button"
							onClick={() => {
								// not platform-safe, probably will break on web but `platform` doesn't have a save dialog option
								save()?.then((result) => {
									if (result) backupKeystore.mutate(result as string);
								});
							}}
						>
							Backup
						</Button>
						<BackupRestoreDialog
							setShowDialog={setShowAlertDialog}
							setDialogData={setAlertDialogData}
							trigger={
								<Button size="sm" variant="gray" className="mr-2">
									Restore
								</Button>
							}
						/>
					</div>
				</SettingsContainer>
				<AlertDialog
					open={showAlertDialog}
					setOpen={setShowAlertDialog}
					title={alertDialogData.title}
					description={alertDialogData.description}
					value={alertDialogData.value}
					inputBox={alertDialogData.inputBox}
				/>
			</>
		);
	}
}

// not sure of a suitable place for this function
export const getCryptoSettings = (
	encryptionAlgorithm: string,
	hashingAlgorithm: string
): [Algorithm, HashingAlgorithm] => {
	const algorithm = encryptionAlgorithm as Algorithm;
	let hashing_algorithm: HashingAlgorithm = { Argon2id: 'Standard' };

	switch (hashingAlgorithm) {
		case 'Argon2id-s':
			hashing_algorithm = { Argon2id: 'Standard' as Params };
			break;
		case 'Argon2id-h':
			hashing_algorithm = { Argon2id: 'Hardened' as Params };
			break;
		case 'Argon2id-p':
			hashing_algorithm = { Argon2id: 'Paranoid' as Params };
			break;
	}

	return [algorithm, hashing_algorithm];
};

// not sure of a suitable place for this function
export const getHashingAlgorithmString = (hashingAlgorithm: HashingAlgorithm): string => {
	let hashing_algorithm = '';

	switch (hashingAlgorithm.Argon2id) {
		case 'Standard':
			hashing_algorithm = 'Argon2id-s';
			break;
		case 'Hardened':
			hashing_algorithm = 'Argon2id-h';
			break;
		case 'Paranoid':
			hashing_algorithm = 'Argon2id-p';
			break;
	}

	return hashing_algorithm;
};
