import { Eye, EyeSlash, Gear, Lock } from 'phosphor-react';
import { useState } from 'react';
import { useLibraryContext, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, ButtonLink, Input, Tabs } from '@sd/ui';
import { showAlertDialog } from '../AlertDialog';
import KeyList from './List';
import KeyMounter from './Mounter';

export function KeyManager() {
	const isUnlocked = useLibraryQuery(['keys.isUnlocked']);

	if (!isUnlocked?.data) return <NotUnlocked />;
	else return <Unlocked />;
}

const Unlocked = () => {
	const { library } = useLibraryContext();

	const unmountAll = useLibraryMutation('keys.unmountAll');
	const clearMasterPassword = useLibraryMutation('keys.clearMasterPassword');

	return (
		<div>
			<Tabs.Root defaultValue="mount">
				<div className="flex flex-col">
					<Tabs.List>
						<Tabs.Trigger className="text-sm font-medium" value="mount">
							Mount
						</Tabs.Trigger>
						<Tabs.Trigger className="text-sm font-medium" value="keys">
							Keys
						</Tabs.Trigger>
						<div className="grow" />
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
						<ButtonLink
							to={`/${library.uuid}/settings/overview`}
							size="icon"
							variant="subtle"
							className="text-ink-faint"
						>
							<Gear className="text-ink-faint h-4 w-4" />
						</ButtonLink>
					</Tabs.List>
				</div>
				<Tabs.Content value="keys">
					<Keys />
				</Tabs.Content>
				<Tabs.Content value="mount">
					<KeyMounter />
				</Tabs.Content>
			</Tabs.Root>
		</div>
	);
};

const Keys = () => {
	const unmountAll = useLibraryMutation(['keys.unmountAll']);

	return (
		<div className="flex h-full max-h-[360px] flex-col">
			<div className="custom-scroll overlay-scroll p-3">
				<div className="">
					{/* <CategoryHeading>Mounted keys</CategoryHeading> */}
					<div className="space-y-1.5">
						<KeyList />
					</div>
				</div>
			</div>
			<div className="border-app-line flex w-full rounded-b-md border-t p-2">
				<Button
					size="sm"
					variant="gray"
					onClick={() => {
						unmountAll.mutate(null);
					}}
				>
					Unmount All
				</Button>
				<div className="grow" />
				<Button size="sm" variant="gray">
					Close
				</Button>
			</div>
		</div>
	);
};

const NotUnlocked = () => {
	const keyringSk = useLibraryQuery(['keys.getSecretKey'], { initialData: '' });
	const unlockKeyManager = useLibraryMutation('keys.unlockKeyManager', {
		onError: () =>
			showAlertDialog({
				title: 'Unlock Error',
				value: 'The information provided to the key manager was incorrect'
			})
	});
	const isKeyManagerUnlocking = useLibraryQuery(['keys.isKeyManagerUnlocking']);

	const [showMasterPassword, setShowMasterPassword] = useState(false);
	const [showSecretKey, setShowSecretKey] = useState(false);

	const [masterPassword, setMasterPassword] = useState('');
	const [secretKey, setSecretKey] = useState('');

	const [enterSkManually, setEnterSkManually] = useState(keyringSk?.data === null);

	const MPCurrentEyeIcon = showMasterPassword ? EyeSlash : Eye;
	const SKCurrentEyeIcon = showSecretKey ? EyeSlash : Eye;

	return (
		<div className="p-2">
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
};
