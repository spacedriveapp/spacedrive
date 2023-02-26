import { useState } from 'react';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, PasswordInput } from '@sd/ui';
import { showAlertDialog } from '~/components/AlertDialog';

export default () => {
	const keyringSk = useLibraryQuery(['keys.getSecretKey'], { initialData: '' });
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

	return (
		<div className="space-y-2 p-2">
			<PasswordInput
				size="sm"
				placeholder="Master Password"
				value={masterPassword}
				onChange={(e) => setMasterPassword(e.target.value)}
				autoFocus
			/>

			{enterSkManually && (
				<PasswordInput
					size="sm"
					placeholder="Secret Key"
					value={secretKey}
					onChange={(e) => setSecretKey(e.target.value)}
					autoFocus
				/>
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
				<p className="text-accent" onClick={() => setEnterSkManually(true)}>
					or enter secret key manually
				</p>
			)}
		</div>
	);
};
