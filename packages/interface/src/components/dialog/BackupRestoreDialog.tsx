import { Eye, EyeSlash } from 'phosphor-react';
import { useState } from 'react';
import { useLibraryMutation } from '@sd/client';
import { Button, Dialog, UseDialogProps, useDialog } from '@sd/ui';
import { forms } from '@sd/ui';
import { usePlatform } from '~/util/Platform';
import { showAlertDialog } from '~/util/dialog';

const { Input, useZodForm, z } = forms;

const schema = z.object({
	masterPassword: z.string(),
	secretKey: z.string(),
	filePath: z.string()
});

export type BackupRestorationDialogProps = UseDialogProps;

export const BackupRestoreDialog = (props: BackupRestorationDialogProps) => {
	const platform = usePlatform();

	const restoreKeystoreMutation = useLibraryMutation('keys.restoreKeystore', {
		onSuccess: (total) => {
			showAlertDialog({
				title: 'Import Successful',
				value: `${total} ${total !== 1 ? 'keys were imported.' : 'key was imported.'}`
			});
		},
		onError: () => {
			showAlertDialog({
				title: 'Import Error',
				value: 'There was an error while restoring your backup.'
			});
		}
	});

	const [show, setShow] = useState({
		masterPassword: false,
		secretKey: false
	});

	const dialog = useDialog(props);

	const MPCurrentEyeIcon = show.masterPassword ? EyeSlash : Eye;
	const SKCurrentEyeIcon = show.secretKey ? EyeSlash : Eye;

	const form = useZodForm({
		schema
	});

	const onSubmit = form.handleSubmit((data) => {
		if (data.filePath !== '') {
			restoreKeystoreMutation.mutate({
				password: data.masterPassword,
				secret_key: data.secretKey,
				path: data.filePath
			});
			form.reset();
		}
	});

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			dialog={dialog}
			title="Restore Keys"
			description="Restore keys from a backup."
			loading={restoreKeystoreMutation.isLoading}
			ctaLabel="Restore"
		>
			<div className="relative mt-3 mb-2 flex grow">
				<Input
					className="grow !py-0.5"
					placeholder="Master Password"
					type={show.masterPassword ? 'text' : 'password'}
					{...form.register('masterPassword', { required: true })}
				/>
				<Button
					onClick={() => setShow((old) => ({ ...old, masterPassword: !old.masterPassword }))}
					size="icon"
					className="absolute right-[5px] top-[5px] border-none"
					type="button"
				>
					<MPCurrentEyeIcon className="h-4 w-4" />
				</Button>
			</div>
			<div className="relative mb-3 flex grow">
				<Input
					className="grow !py-0.5"
					placeholder="Secret Key"
					type={show.secretKey ? 'text' : 'password'}
					{...form.register('secretKey')}
				/>
				<Button
					onClick={() => setShow((old) => ({ ...old, secretKey: !old.secretKey }))}
					size="icon"
					className="absolute right-[5px] top-[5px] border-none"
				>
					<SKCurrentEyeIcon className="h-4 w-4" />
				</Button>
			</div>
			<div className="relative mb-2 flex grow">
				<Button
					size="sm"
					variant={form.watch('filePath') !== '' ? 'accent' : 'gray'}
					type="button"
					onClick={() => {
						if (!platform.openFilePickerDialog) {
							// TODO: Support opening locations on web
							showAlertDialog({
								title: 'Error',
								value: "System dialogs aren't supported on this platform."
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
	);
};
