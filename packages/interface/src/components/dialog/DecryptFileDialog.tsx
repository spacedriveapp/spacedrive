import { RadioGroup } from '@headlessui/react';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, Dialog, Input, Switch } from '@sd/ui';
import { Eye, EyeSlash, Info } from 'phosphor-react';
import { useState } from 'react';

import { usePlatform } from '../../util/Platform';
import { Tooltip } from '../tooltip/Tooltip';
import { GenericAlertDialogProps } from './AlertDialog';

interface DecryptDialogProps {
	open: boolean;
	setOpen: (isShowing: boolean) => void;
	location_id: number | null;
	object_id: number | null;
	setAlertDialogData: (data: GenericAlertDialogProps) => void;
}

export const DecryptFileDialog = (props: DecryptDialogProps) => {
	const platform = usePlatform();
	const { location_id, object_id } = props;
	const decryptFile = useLibraryMutation('files.decryptFiles');
	const [outputPath, setOutputpath] = useState('');
	const [password, setPassword] = useState('');
	const [saveToKeyManager, setSaveToKeyManager] = useState(true);
	const [showPassword, setShowPassword] = useState(false);
	const PasswordCurrentEyeIcon = showPassword ? EyeSlash : Eye;

	const mountedUuids = useLibraryQuery(['keys.listMounted'], {
		onSuccess: (data) => {
			hasMountedKeys = data.length > 0 ? true : false;
			if (!hasMountedKeys) {
				setDecryptType('password');
			} else {
				setDecryptType('key');
			}
		}
	});

	let hasMountedKeys =
		mountedUuids.data !== undefined && mountedUuids.data.length > 0 ? true : false;

	const [decryptType, setDecryptType] = useState(hasMountedKeys ? 'key' : 'password');

	return (
		<>
			<Dialog
				open={props.open}
				setOpen={props.setOpen}
				title="Decrypt a file"
				description="Leave the output file blank for the default."
				loading={decryptFile.isLoading}
				ctaLabel="Decrypt"
				ctaAction={() => {
					const output = outputPath !== '' ? outputPath : null;
					const pw = decryptType === 'password' ? password : null;
					const save = decryptType === 'password' ? saveToKeyManager : null;

					props.setOpen(false);

					location_id &&
						object_id &&
						decryptFile.mutate(
							{
								location_id,
								object_id,
								output_path: output,
								password: pw,
								save_to_library: save
							},
							{
								onSuccess: () => {
									props.setAlertDialogData({
										open: true,
										title: 'Info',
										value:
											'The decryption job has started successfully. You may track the progress in the job overview panel.',
										inputBox: false,
										description: ''
									});
								},
								onError: () => {
									props.setAlertDialogData({
										open: true,
										title: 'Error',
										value: 'The decryption job failed to start.',
										inputBox: false,
										description: ''
									});
								}
							}
						);
				}}
			>
				<RadioGroup value={decryptType} onChange={setDecryptType} className="mt-2">
					<span className="text-xs font-bold">Key Type</span>
					<div className="flex flex-row gap-2 mt-2">
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

				{decryptType === 'password' && (
					<>
						<div className="relative flex flex-grow mt-3 mb-2">
							<Input
								className={`flex-grow w-max !py-0.5`}
								placeholder="Password"
								onChange={(e) => setPassword(e.target.value)}
								value={password}
								type={showPassword ? 'text' : 'password'}
								required
							/>
							<Button
								onClick={() => setShowPassword(!showPassword)}
								size="icon"
								className="border-none absolute right-[5px] top-[5px]"
								type="button"
							>
								<PasswordCurrentEyeIcon className="w-4 h-4" />
							</Button>
						</div>

						<div className="relative flex flex-grow mt-3 mb-2">
							<div className="space-x-2">
								<Switch
									className="bg-app-selected"
									size="sm"
									checked={saveToKeyManager}
									onCheckedChange={setSaveToKeyManager}
								/>
							</div>
							<span className="ml-3 text-xs font-medium mt-0.5">Save to Key Manager</span>
							<Tooltip label="This key will be saved to the key manager">
								<Info className="w-4 h-4 ml-1.5 text-ink-faint mt-0.5" />
							</Tooltip>
						</div>
					</>
				)}

				<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
					<div className="flex flex-col">
						<span className="text-xs font-bold">Output file</span>

						<Button
							size="sm"
							variant={outputPath !== '' ? 'accent' : 'gray'}
							className="h-[23px] text-xs leading-3 mt-2"
							type="button"
							onClick={() => {
								// if we allow the user to encrypt multiple files simultaneously, this should become a directory instead
								if (!platform.saveFilePickerDialog) {
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
								platform.saveFilePickerDialog().then((result) => {
									if (result) setOutputpath(result as string);
								});
							}}
						>
							Select
						</Button>
					</div>
				</div>
			</Dialog>
		</>
	);
};
