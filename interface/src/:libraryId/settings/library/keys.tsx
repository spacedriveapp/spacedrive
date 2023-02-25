import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import clsx from 'clsx';
import { Eye, EyeSlash, Lock, Plus } from 'phosphor-react';
import { PropsWithChildren, useState } from 'react';
import QRCode from 'react-qr-code';
import { animated, useTransition } from 'react-spring';
import { HashingAlgorithm, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, Input, dialogManager } from '@sd/ui';
import { BackupRestoreDialog } from '~/components/dialog/BackupRestoreDialog';
import { KeyViewerDialog } from '~/components/dialog/KeyViewerDialog';
import { MasterPasswordChangeDialog } from '~/components/dialog/MasterPasswordChangeDialog';
import { ListOfKeys } from '~/components/key/KeyList';
import { KeyMounter } from '~/components/key/KeyMounter';
import { SettingsSubHeader } from '~/components/settings/SettingsSubHeader';
import { usePlatform } from '~/util/Platform';
import { showAlertDialog } from '~/util/dialog';
import { Header } from '../Layout';

interface Props extends DropdownMenu.MenuContentProps {
	trigger: React.ReactNode;
	transformOrigin?: string;
	disabled?: boolean;
}

export const KeyMounterDropdown = ({
	trigger,
	children,
	transformOrigin,
	className
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
										'cursor-default select-none rounded-lg',
										'text-ink text-left text-sm',
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
	const platform = usePlatform();
	const isUnlocked = useLibraryQuery(['keys.isUnlocked']);
	const keyringSk = useLibraryQuery(['keys.getSecretKey'], { initialData: '' }); // assume true by default, as it will often be the case. need to fix this with an rspc subscription+such
	const unlockKeyManager = useLibraryMutation('keys.unlockKeyManager', {
		onError: () => {
			showAlertDialog({
				title: 'Unlock Error',
				value: 'The information provided to the key manager was incorrect'
			});
		}
	});

	const unmountAll = useLibraryMutation('keys.unmountAll');
	const clearMasterPassword = useLibraryMutation('keys.clearMasterPassword');
	const backupKeystore = useLibraryMutation('keys.backupKeystore');
	const isKeyManagerUnlocking = useLibraryQuery(['keys.isKeyManagerUnlocking']);

	const [showMasterPassword, setShowMasterPassword] = useState(false);
	const [showSecretKey, setShowSecretKey] = useState(false);
	const [masterPassword, setMasterPassword] = useState('');
	const [secretKey, setSecretKey] = useState(''); // for the unlock form
	const [viewSecretKey, setViewSecretKey] = useState(false); // for the settings page

	const keys = useLibraryQuery(['keys.list']);

	const MPCurrentEyeIcon = showMasterPassword ? EyeSlash : Eye;
	const SKCurrentEyeIcon = showSecretKey ? EyeSlash : Eye;

	const [enterSkManually, setEnterSkManually] = useState(keyringSk?.data === null);

	if (!isUnlocked?.data) {
		return (
			<div className="mx-20 mt-10 p-2">
				<div className="relative mb-2 flex grow">
					<Input
						value={masterPassword}
						onChange={(e) => setMasterPassword(e.target.value)}
						autoFocus
						type={showMasterPassword ? 'text' : 'password'}
						className="grow !py-0.5"
						placeholder="Master Password"
					/>
					<Button
						onClick={() => setShowMasterPassword(!showMasterPassword)}
						size="icon"
						className="absolute right-[5px] top-[5px] border-none"
					>
						<MPCurrentEyeIcon className="h-4 w-4" />
					</Button>
				</div>
				{enterSkManually && (
					<div className="relative mb-2 flex grow">
						<Input
							value={secretKey}
							onChange={(e) => setSecretKey(e.target.value)}
							type={showSecretKey ? 'text' : 'password'}
							className="grow !py-0.5"
							placeholder="Secret Key"
						/>
						<Button
							onClick={() => setShowSecretKey(!showSecretKey)}
							size="icon"
							className="absolute right-[5px] top-[5px] border-none"
						>
							<SKCurrentEyeIcon className="h-4 w-4" />
						</Button>
					</div>
				)}

				<Button
					className="w-full"
					variant="accent"
					disabled={
						unlockKeyManager.isLoading || isKeyManagerUnlocking.data !== null
							? isKeyManagerUnlocking.data!
							: false
					}
					onClick={() => {
						if (masterPassword !== '') {
							setMasterPassword('');
							setSecretKey('');
							unlockKeyManager.mutate({ password: masterPassword, secret_key: secretKey });
						}
					}}
				>
					Unlock
				</Button>
				{!enterSkManually && (
					<div className="relative flex grow">
						<p className="text-accent mt-2" onClick={() => setEnterSkManually(true)}>
							or enter secret key manually
						</p>
					</div>
				)}
			</div>
		);
	} else {
		return (
			<>
				<Header
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
								variant="subtle"
								className="text-ink-faint"
							>
								<Lock className="text-ink-faint h-4 w-4" />
							</Button>
							<KeyMounterDropdown
								trigger={
									<Button size="icon" variant="subtle" className="text-ink-faint">
										<Plus className="text-ink-faint h-4 w-4" />
									</Button>
								}
							>
								<KeyMounter />
							</KeyMounterDropdown>
						</div>
					}
				/>

				{isUnlocked && (
					<div className="grid space-y-2">
						<ListOfKeys />
					</div>
				)}

				{keyringSk?.data && (
					<>
						<SettingsSubHeader title="Secret key" />
						{!viewSecretKey && (
							<div className="flex flex-row">
								<Button size="sm" variant="gray" onClick={() => setViewSecretKey(true)}>
									View Secret Key
								</Button>
							</div>
						)}
						{viewSecretKey && (
							<div
								className="flex flex-row"
								onClick={() => {
									keyringSk.data && navigator.clipboard.writeText(keyringSk.data);
								}}
							>
								<>
									<QRCode size={128} value={keyringSk.data} />
									<p className="mt-14 ml-6 text-xl font-bold">{keyringSk.data}</p>
								</>
							</div>
						)}
					</>
				)}

				<SettingsSubHeader title="Password Options" />
				<div className="flex flex-row">
					<Button
						size="sm"
						variant="gray"
						className="mr-2"
						onClick={() => dialogManager.create((dp) => <MasterPasswordChangeDialog {...dp} />)}
					>
						Change Master Password
					</Button>
					<Button
						size="sm"
						variant="gray"
						className="mr-2"
						hidden={keys.data?.length === 0}
						onClick={() => dialogManager.create((dp) => <KeyViewerDialog {...dp} />)}
					>
						View Key Values
					</Button>
				</div>

				<SettingsSubHeader title="Data Recovery" />
				<div className="flex flex-row">
					<Button
						size="sm"
						variant="gray"
						className="mr-2"
						type="button"
						onClick={() => {
							if (!platform.saveFilePickerDialog) {
								// TODO: Support opening locations on web
								showAlertDialog({
									title: 'Error',
									value: "System dialogs aren't supported on this platform."
								});
								return;
							}
							platform.saveFilePickerDialog().then((result) => {
								if (result) backupKeystore.mutate(result as string);
							});
						}}
					>
						Backup
					</Button>
					<Button
						size="sm"
						variant="gray"
						className="mr-2"
						onClick={() => dialogManager.create((dp) => <BackupRestoreDialog {...dp} />)}
					>
						Restore
					</Button>
				</div>
			</>
		);
	}
}

const HASHING_ALGOS = {
	'Argon2id-s': { name: 'Argon2id', params: 'Standard' },
	'Argon2id-h': { name: 'Argon2id', params: 'Hardened' },
	'Argon2id-p': { name: 'Argon2id', params: 'Paranoid' },
	'BalloonBlake3-s': { name: 'BalloonBlake3', params: 'Standard' },
	'BalloonBlake3-h': { name: 'BalloonBlake3', params: 'Hardened' },
	'BalloonBlake3-p': { name: 'BalloonBlake3', params: 'Paranoid' }
} as const satisfies Record<string, HashingAlgorithm>;

// not sure of a suitable place for this function
export const getHashingAlgorithmSettings = (hashingAlgorithm: keyof typeof HASHING_ALGOS): HashingAlgorithm => {
	return HASHING_ALGOS[hashingAlgorithm] || { name: 'Argon2id', params: 'Standard' };
};

// not sure of a suitable place for this function
export const getHashingAlgorithmString = (hashingAlgorithm: HashingAlgorithm): string => {
	return Object.entries(HASHING_ALGOS).find(
		([_, hashAlg]) =>
			hashAlg.name === hashingAlgorithm.name && hashAlg.params === hashingAlgorithm.params
	)![0];
};
