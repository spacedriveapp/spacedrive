import { Algorithm, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, Dialog, Select, SelectOption } from '@sd/ui';
import { useState } from 'react';
import { useForm } from 'react-hook-form';

import { getHashingAlgorithmString } from '../../screens/settings/library/KeysSetting';
import { usePlatform } from '../../util/Platform';
import { SelectOptionKeyList } from '../key/KeyList';
import { Checkbox } from '../primitive/Checkbox';
import { GenericAlertDialogProps } from './AlertDialog';

interface EncryptDialogProps {
	open: boolean;
	setOpen: (isShowing: boolean) => void;
	location_id: number | null;
	path_id: number | undefined;
	setAlertDialogData: (data: GenericAlertDialogProps) => void;
}

type FormValues = {
	key: string;
	encryptionAlgo: string;
	hashingAlgo: string;
	metadata: boolean;
	previewMedia: boolean;
	outputPath: string;
};

export const EncryptFileDialog = (props: EncryptDialogProps) => {
	const platform = usePlatform();

	const UpdateKey = (uuid: string) => {
		form.setValue('key', uuid);
		const hashAlg = keys.data?.find((key) => {
			return key.uuid === uuid;
		})?.hashing_algorithm;
		hashAlg && form.setValue('hashingAlgo', getHashingAlgorithmString(hashAlg));
	};

	const keys = useLibraryQuery(['keys.list']);
	const mountedUuids = useLibraryQuery(['keys.listMounted'], {
		onSuccess: (data) => {
			UpdateKey(data[0]);
		}
	});

	const encryptFile = useLibraryMutation('files.encryptFiles', {
		onSuccess: () => {
			props.setAlertDialogData({
				open: true,
				title: 'Success',
				value:
					'The encryption job has started successfully. You may track the progress in the job overview panel.',
				inputBox: false,
				description: ''
			});
		},
		onError: () => {
			props.setAlertDialogData({
				open: true,
				title: 'Error',
				value: 'The encryption job failed to start.',
				inputBox: false,
				description: ''
			});
		}
	});

	const form = useForm<FormValues>({
		defaultValues: {
			key: '',
			encryptionAlgo: 'XChaCha20Poly1305',
			hashingAlgo: 'Argon2id-s',
			metadata: false,
			previewMedia: false,
			outputPath: ''
		}
	});

	const onSubmit = form.handleSubmit((data) => {
		const output = data.outputPath !== '' ? data.outputPath : null;
		props.setOpen(false);

		props.location_id &&
			props.path_id &&
			encryptFile.mutate({
				algorithm: data.encryptionAlgo as Algorithm,
				key_uuid: data.key,
				location_id: props.location_id,
				path_id: props.path_id,
				metadata: data.metadata,
				preview_media: data.previewMedia,
				output_path: output
			});

		form.reset();
	});

	return (
		<>
			<Dialog
				open={props.open}
				setOpen={props.setOpen}
				title="Encrypt a file"
				description="Configure your encryption settings. Leave the output file blank for the default."
				loading={encryptFile.isLoading}
				ctaLabel="Encrypt"
			>
				<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
					<div className="flex flex-col">
						<span className="text-xs font-bold">Key</span>
						<Select
							className="mt-2"
							value={form.watch('key')}
							onChange={(e) => {
								UpdateKey(e);
							}}
						>
							{mountedUuids.data && <SelectOptionKeyList keys={mountedUuids.data} />}
						</Select>
					</div>
					<div className="flex flex-col">
						<span className="text-xs font-bold">Output file</span>

						<Button
							size="sm"
							variant={form.watch('outputPath') !== '' ? 'accent' : 'gray'}
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
									if (result) form.setValue('outputPath', result as string);
								});
							}}
						>
							Select
						</Button>
					</div>
				</div>

				<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
					<div className="flex flex-col">
						<span className="text-xs font-bold">Encryption</span>
						<Select
							className="mt-2"
							value={form.watch('encryptionAlgo')}
							onChange={(e) => form.setValue('encryptionAlgo', e)}
						>
							<SelectOption value="XChaCha20Poly1305">XChaCha20-Poly1305</SelectOption>
							<SelectOption value="Aes256Gcm">AES-256-GCM</SelectOption>
						</Select>
					</div>
					<div className="flex flex-col">
						<span className="text-xs font-bold">Hashing</span>
						<Select
							className="mt-2 text-gray-400/80"
							onChange={() => {}}
							disabled
							value={form.watch('hashingAlgo')}
						>
							<SelectOption value="Argon2id-s">Argon2id (standard)</SelectOption>
							<SelectOption value="Argon2id-h">Argon2id (hardened)</SelectOption>
							<SelectOption value="Argon2id-p">Argon2id (paranoid)</SelectOption>
							<SelectOption value="BalloonBlake3-s">BLAKE3-Balloon (standard)</SelectOption>
							<SelectOption value="BalloonBlake3-h">BLAKE3-Balloon (hardened)</SelectOption>
							<SelectOption value="BalloonBlake3-p">BLAKE3-Balloon (paranoid)</SelectOption>
						</Select>
					</div>
				</div>

				<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
					<div className="flex">
						<span className="text-sm font-bold mr-3 ml-0.5 mt-0.5">Metadata</span>
						<Checkbox
							checked={form.watch('metadata')}
							onChange={(e) => form.setValue('metadata', e.target.checked)}
						/>
					</div>
					<div className="flex">
						<span className="text-sm font-bold mr-3 ml-0.5 mt-0.5">Preview Media</span>
						<Checkbox
							checked={form.watch('previewMedia')}
							onChange={(e) => form.setValue('previewMedia', e.target.checked)}
						/>
					</div>
				</div>
			</Dialog>
		</>
	);
};
