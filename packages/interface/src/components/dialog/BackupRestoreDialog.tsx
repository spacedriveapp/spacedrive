import { useLibraryMutation } from '@sd/client';
import { Button, Dialog, Input } from '@sd/ui';
import { Eye, EyeSlash } from 'phosphor-react';
import { ReactNode, useState } from 'react';
import { SubmitHandler, useForm } from 'react-hook-form';

import { usePlatform } from '../../util/Platform';
import { GenericAlertDialogProps } from './AlertDialog';

type FormValues = {
	masterPassword: string;
	secretKey: string;
	filePath: string;
};

export interface BackupRestorationDialogProps {
	trigger: ReactNode;
	setAlertDialogData: (data: GenericAlertDialogProps) => void;
}

export const BackupRestoreDialog = (props: BackupRestorationDialogProps) => {
	const platform = usePlatform();
	const form = useForm<FormValues>({
		defaultValues: {
			masterPassword: '',
			secretKey: '',
			filePath: ''
		}
	});

	const onSubmit: SubmitHandler<FormValues> = (data) => {
		const sk = data.secretKey === '' ? null : data.secretKey;

		if (data.filePath !== '') {
			restoreKeystoreMutation.mutate(
				{
					password: data.masterPassword,
					secret_key: sk,
					path: data.filePath
				},
				{
					onSuccess: (total) => {
						setShowBackupRestoreDialog(false);
						props.setAlertDialogData({
							open: true,
							title: 'Import Successful',
							description: '',
							value: `${total} ${total !== 1 ? 'keys were imported.' : 'key was imported.'}`,
							inputBox: false
						});
					},
					onError: () => {
						setShowBackupRestoreDialog(false);
						props.setAlertDialogData({
							open: true,
							title: 'Import Error',
							description: '',
							value: 'There was an error while restoring your backup.',
							inputBox: false
						});
					}
				}
			);
			form.reset();
		}
	};

	const [showBackupRestoreDialog, setShowBackupRestoreDialog] = useState(false);
	const restoreKeystoreMutation = useLibraryMutation('keys.restoreKeystore');

	const [showMasterPassword, setShowMasterPassword] = useState(false);
	const [showSecretKey, setShowSecretKey] = useState(false);

	const MPCurrentEyeIcon = showMasterPassword ? EyeSlash : Eye;
	const SKCurrentEyeIcon = showSecretKey ? EyeSlash : Eye;

	return (
		<>
			<form onSubmit={form.handleSubmit(onSubmit)}>
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
							{...form.register('masterPassword', { required: true })}
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
							{...form.register('secretKey', { required: false })}
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
							variant={form.watch('filePath') !== '' ? 'accent' : 'gray'}
							type="button"
							onClick={() => {
								if (!platform.openFilePickerDialog) {
									// TODO: Support opening locations on web
									props.setAlertDialogData({
										open: true,
										title: 'Error',
										description: '',
										value: "System dialogs aren't supported on this platform.",
										inputBox: false
									});
									return;
								}
								platform.openFilePickerDialog().then((result) => {
									if (result) form.setValue('filePath', result as string);
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
