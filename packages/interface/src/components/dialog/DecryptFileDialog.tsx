import { RadioGroup } from '@headlessui/react';
import { Eye, EyeSlash, Info } from 'phosphor-react';
import { useState } from 'react';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, Dialog, UseDialogProps, useDialog } from '@sd/ui';
import { Input, Switch, useZodForm, z } from '@sd/ui/src/forms';
import { showAlertDialog } from '~/util/dialog';
import { usePlatform } from '../../util/Platform';
import { Tooltip } from '../tooltip/Tooltip';

interface DecryptDialogProps extends UseDialogProps {
	location_id: number;
	path_id: number;
}

const schema = z.object({
	type: z.union([z.literal('password'), z.literal('key')]),
	outputPath: z.string(),
	password: z.string(),
	saveToKeyManager: z.boolean()
});

export const DecryptFileDialog = (props: DecryptDialogProps) => {
	const platform = usePlatform();
	const dialog = useDialog(props);

	const mountedUuids = useLibraryQuery(['keys.listMounted'], {
		onSuccess: (data) => {
			hasMountedKeys = data.length > 0 ? true : false;
			if (!hasMountedKeys) {
				form.setValue('type', 'password');
			} else {
				form.setValue('type', 'key');
			}
		}
	});

	let hasMountedKeys =
		mountedUuids.data !== undefined && mountedUuids.data.length > 0 ? true : false;

	const decryptFile = useLibraryMutation('files.decryptFiles', {
		onSuccess: () => {
			showAlertDialog({
				title: 'Success',
				value:
					'The decryption job has started successfully. You may track the progress in the job overview panel.'
			});
		},
		onError: () => {
			showAlertDialog({
				title: 'Error',
				value: 'The decryption job failed to start.'
			});
		}
	});

	const [show, setShow] = useState({ password: false });

	const PasswordCurrentEyeIcon = show.password ? EyeSlash : Eye;

	const form = useZodForm({
		defaultValues: {
			type: hasMountedKeys ? 'key' : 'password',
			saveToKeyManager: true,
			outputPath: '',
			password: ''
		},
		schema
	});

	const onSubmit = form.handleSubmit((data) =>
		decryptFile.mutateAsync({
			location_id: props.location_id,
			path_id: props.path_id,
			output_path: data.outputPath !== '' ? data.outputPath : null,
			password: data.type === 'password' ? data.password : null,
			save_to_library: data.type === 'password' ? data.saveToKeyManager : null
		})
	);

	return (
		<Dialog
			form={form}
			dialog={dialog}
			onSubmit={onSubmit}
			title="Decrypt a file"
			description="Leave the output file blank for the default."
			loading={decryptFile.isLoading}
			ctaLabel="Decrypt"
		>
			<RadioGroup
				value={form.watch('type')}
				onChange={(e: 'key' | 'password') => form.setValue('type', e)}
				className="mt-2"
			>
				<span className="text-xs font-bold">Key Type</span>
				<div className="mt-2 flex flex-row gap-2">
					<RadioGroup.Option disabled={!hasMountedKeys} value="key">
						{({ checked }) => (
							<Button
								type="button"
								disabled={!hasMountedKeys}
								size="sm"
								variant={checked ? 'accent' : 'gray'}
							>
								Key Manager
							</Button>
						)}
					</RadioGroup.Option>
					<RadioGroup.Option value="password">
						{({ checked }) => (
							<Button type="button" size="sm" variant={checked ? 'accent' : 'gray'}>
								Password
							</Button>
						)}
					</RadioGroup.Option>
				</div>
			</RadioGroup>

			{form.watch('type') === 'password' && (
				<>
					<div className="relative mt-3 mb-2 flex grow">
						<Input
							className={`w-max grow !py-0.5`}
							placeholder="Password"
							type={show.password ? 'text' : 'password'}
							{...form.register('password', { required: true })}
						/>
						<Button
							onClick={() => setShow((old) => ({ ...old, password: !old.password }))}
							size="icon"
							className="absolute right-[5px] top-[5px] border-none"
							type="button"
						>
							<PasswordCurrentEyeIcon className="h-4 w-4" />
						</Button>
					</div>

					<div className="relative mt-3 mb-2 flex grow">
						<div className="space-x-2">
							<Switch
								className="bg-app-selected"
								size="sm"
								{...form.register('saveToKeyManager')}
							/>
						</div>
						<span className="ml-3 mt-0.5 text-xs font-medium">Save to Key Manager</span>
						<Tooltip label="This key will be saved to the key manager">
							<Info className="text-ink-faint ml-1.5 mt-0.5 h-4 w-4" />
						</Tooltip>
					</div>
				</>
			)}

			<div className="mt-4 mb-3 grid w-full grid-cols-2 gap-4">
				<div className="flex flex-col">
					<span className="text-xs font-bold">Output file</span>

					<Button
						size="sm"
						variant={form.watch('outputPath') !== '' ? 'accent' : 'gray'}
						className="mt-2 h-[23px] text-xs leading-3"
						type="button"
						onClick={() => {
							// if we allow the user to encrypt multiple files simultaneously, this should become a directory instead
							if (!platform.saveFilePickerDialog) {
								// TODO: Support opening locations on web
								showAlertDialog({
									title: 'Error',
									value: "System dialogs aren't supported on this platform."
								});
								return;
							}
							platform.saveFilePickerDialog().then((result) => {
								if (result) form.setValue('outputPath', result as string);
							});
						}}
					>
						Select
					</Button>
				</div>
			</div>
		</Dialog>
	);
};
