import { useLibraryMutation } from '@sd/client';
import { Button, Dialog, Input } from '@sd/ui';
import { open } from '@tauri-apps/api/dialog';
import { Eye, EyeSlash } from 'phosphor-react';
import { ReactNode, useState } from 'react';
import { SubmitHandler, useForm } from 'react-hook-form';

import { GenericAlertDialogProps } from './AlertDialog';

type FormValues = {
	masterPassword: string;
	secretKey: string;
};

export interface BackupRestorationDialogProps {
	trigger: ReactNode;
	setShowDialog: (isShowing: boolean) => void;
	setDialogData: (data: GenericAlertDialogProps) => void;
}

export const BackupRestoreDialog = (props: BackupRestorationDialogProps) => {
	const { register, handleSubmit, getValues, setValue } = useForm<FormValues>({
		defaultValues: {
			masterPassword: '',
			secretKey: ''
		}
	});

	const onSubmit: SubmitHandler<FormValues> = (data) => {
		if (filePath !== '') {
			restoreKeystoreMutation.mutate(
				{
					password: data.masterPassword,
					secret_key: data.secretKey,
					path: filePath
				},
				{
					onSuccess: (total) => {
						setShowBackupRestoreDialog(false);
						props.setDialogData({
							open: true,
							title: 'Import Successful',
							description: '',
							value: `${total} ${total !== 1 ? 'keys were imported.' : 'key was imported.'}`,
							inputBox: false
						});
					},
					onError: () => {
						setShowBackupRestoreDialog(false);
						props.setDialogData({
							open: true,
							title: 'Import Error',
							description: '',
							value: 'There was an error while restoring your backup.',
							inputBox: false
						});
					}
				}
			);
			setValue('masterPassword', '');
			setValue('secretKey', '');
			setFilePath('');
		}
	};

	const [showBackupRestoreDialog, setShowBackupRestoreDialog] = useState(false);
	const restoreKeystoreMutation = useLibraryMutation('keys.restoreKeystore');

	const [showMasterPassword, setShowMasterPassword] = useState(false);
	const [showSecretKey, setShowSecretKey] = useState(false);
	const [filePath, setFilePath] = useState('');

	const MPCurrentEyeIcon = showMasterPassword ? EyeSlash : Eye;
	const SKCurrentEyeIcon = showSecretKey ? EyeSlash : Eye;

	return (
		<>
			<form onSubmit={handleSubmit(onSubmit)}>
				<Dialog
					open={showBackupRestoreDialog}
					setOpen={setShowBackupRestoreDialog}
					title="Restore Keys"
					description="Restore keys from a backup."
					loading={restoreKeystoreMutation.isLoading}
					ctaLabel="Restore"
					trigger={props.trigger}
				>
					<div className="relative flex flex-grow mt-3 mb-2">
						<Input
							className="flex-grow !py-0.5"
							placeholder="Master Password"
							required
							type={showMasterPassword ? 'text' : 'password'}
							{...register('masterPassword', { required: true })}
						/>
						<Button
							onClick={() => setShowMasterPassword(!showMasterPassword)}
							size="icon"
							className="border-none absolute right-[5px] top-[5px]"
							type="button"
						>
							<MPCurrentEyeIcon className="w-4 h-4" />
						</Button>
					</div>
					<div className="relative flex flex-grow mb-3">
						<Input
							className="flex-grow !py-0.5"
							placeholder="Secret Key"
							{...register('secretKey', { required: true })}
							required
							type={showSecretKey ? 'text' : 'password'}
						/>
						<Button
							onClick={() => setShowSecretKey(!showSecretKey)}
							size="icon"
							className="border-none absolute right-[5px] top-[5px]"
							type="button"
						>
							<SKCurrentEyeIcon className="w-4 h-4" />
						</Button>
					</div>
					<div className="relative flex flex-grow mb-2">
						<Button
							size="sm"
							variant={filePath !== '' ? 'accent' : 'gray'}
							type="button"
							onClick={() => {
								open()?.then((result) => {
									if (result) setFilePath(result as string);
								});
							}}
						>
							Select File
						</Button>
					</div>
				</Dialog>
			</form>
		</>
	);
};
