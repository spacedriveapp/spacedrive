import { Spinner } from 'phosphor-react';
import { useState } from 'react';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, PasswordInput } from '@sd/ui';
import { showAlertDialog } from '~/components/AlertDialog';

// TODO: Should be a form
export default () => {
	const keyringSk = useLibraryQuery(['keys.getSecretKey'], { initialData: '' });
	const isUnlocked = useLibraryQuery(['keys.isUnlocked']);
	const unlockKeyManager = useLibraryMutation('keys.unlockKeyManager', {
		onError: () =>
			showAlertDialog({
				title: 'Unlock Error',
				value: 'The information provided to the key manager was incorrect'
			})
	});
	const isKeyManagerUnlocking = useLibraryQuery(['keys.isKeyManagerUnlocking']);

	const [masterPassword, setMasterPassword] = useState('');
	const [secretKey, setSecretKey] = useState('');

	const [enterSkManually, setEnterSkManually] = useState(keyringSk?.data === null);

	const isUnlocking = unlockKeyManager.isLoading || isKeyManagerUnlocking.data == null;

	return (
		<div className="w-[350px] space-y-2 p-4">
			<PasswordInput
				placeholder="Master Password"
				value={masterPassword}
				onChange={(e) => setMasterPassword(e.target.value)}
				autoFocus
			/>

			{enterSkManually && (
				<PasswordInput
					placeholder="Secret Key"
					value={secretKey}
					onChange={(e) => setSecretKey(e.target.value)}
					autoFocus
				/>
			)}

			<Button
				className="w-full"
				variant="accent"
				disabled={isUnlocking}
				onClick={() => {
					if (masterPassword !== '') {
						setMasterPassword('');
						setSecretKey('');

						// TODO: Catch error
						unlockKeyManager
							.mutateAsync({
								password: masterPassword,
								secret_key: secretKey
							})
							.then(() => isUnlocked.refetch());
					}
				}}
			>
				{isUnlocking ? (
					<Spinner className="mx-auto h-6 w-6 animate-spin fill-white text-white text-opacity-40" />
				) : (
					'Unlock'
				)}
			</Button>

			{!enterSkManually && (
				<Button
					href="#"
					onClick={() => setEnterSkManually(true)}
					className="!pointer-events-auto text-accent"
				>
					or enter secret key manually
				</Button>
			)}
		</div>
	);
};
