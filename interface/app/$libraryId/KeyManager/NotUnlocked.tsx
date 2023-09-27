// import { Spinner } from '@phosphor-icons/react';
// import { useState } from 'react';
// import { useLibraryMutation, useLibraryQuery } from '@sd/client';
// import { Button, PasswordInput } from '@sd/ui';
// import { showAlertDialog } from '~/components';

// // TODO: Should be a form
// export default () => {
// 	const keyringSk = useLibraryQuery(['keys.getSecretKey'], { initialData: '' });
// 	const isUnlocked = useLibraryQuery(['keys.isUnlocked']);
// 	const unlockKeyManager = useLibraryMutation('keys.unlockKeyManager', {
// 		onError: () =>
// 			showAlertDialog({
// 				title: 'Unlock Error',
// 				value: 'The information provided to the key manager was incorrect'
// 			})
// 	});
// 	const isKeyManagerUnlocking = useLibraryQuery(['keys.isKeyManagerUnlocking']);

// 	const [masterPassword, setMasterPassword] = useState('');

// 	const isUnlocking = unlockKeyManager.isLoading || isKeyManagerUnlocking.data == null;

// 	return (
// 		<div className="w-[350px] space-y-2 p-4">
// 			<PasswordInput
// 				placeholder="Master Password"
// 				value={masterPassword}
// 				onChange={(e) => setMasterPassword(e.target.value)}
// 				autoFocus
// 				disabled={isUnlocking}
// 			/>

// 			<Button
// 				className="w-full"
// 				variant="accent"
// 				disabled={isUnlocking}
// 				onClick={() => {
// 					if (masterPassword !== '' && keyringSk.data) {
// 						unlockKeyManager
// 							.mutateAsync({
// 								password: masterPassword,
// 								secret_key: keyringSk.data
// 							})
// 							.then(() => isUnlocked.refetch());
// 					}
// 				}}
// 			>
// 				{isUnlocking ? (
// 					<Spinner className="mx-auto h-5 w-5 animate-spin fill-white text-white text-opacity-40" />
// 				) : (
// 					'Unlock'
// 				)}
// 			</Button>
// 		</div>
// 	);
// };
