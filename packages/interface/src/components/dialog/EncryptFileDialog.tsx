import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, Dialog, Select, SelectOption } from '@sd/ui';
import { useState } from 'react';

import {
	getCryptoSettings,
	getHashingAlgorithmString
} from '../../screens/settings/library/KeysSetting';
import { usePlatform } from '../../util/Platform';
import { SelectOptionKeyList } from '../key/KeyList';
import { Checkbox } from '../primitive/Checkbox';
import { GenericAlertDialogProps } from './AlertDialog';

interface EncryptDialogProps {
	open: boolean;
	setOpen: (isShowing: boolean) => void;
	location_id: number | null;
	object_id: number | null;
	setAlertDialogData: (data: GenericAlertDialogProps) => void;
}

export const EncryptFileDialog = (props: EncryptDialogProps) => {
	const platform = usePlatform();
	const { location_id, object_id } = props;
	const keys = useLibraryQuery(['keys.list']);
	const mountedUuids = useLibraryQuery(['keys.listMounted'], {
		onSuccess: (data) => {
			if (key === '' && data.length !== 0) {
				// when this query updates and a key is officially mounted, update `key` (the user shouldn't be able to see this dialog before a key is mounted)
				// only update if no key is currently set
				UpdateKey(data[0]);
			}
		}
	});

	const UpdateKey = (uuid: string) => {
		setKey(uuid);
		const hashAlg = keys.data?.find((key) => {
			return key.uuid === uuid;
		})?.hashing_algorithm;
		hashAlg && setHashingAlgo(getHashingAlgorithmString(hashAlg));
	};

	const encryptFile = useLibraryMutation('files.encryptFiles');

	// the selected key will be random, we should prioritise the default
	const [key, setKey] = useState('');

	// decided against react-hook-form, as it doesn't allow us to work with select boxes and such
	const [metadata, setMetadata] = useState(false);
	const [previewMedia, setPreviewMedia] = useState(false);
	const [encryptionAlgo, setEncryptionAlgo] = useState('XChaCha20Poly1305');
	const [hashingAlgo, setHashingAlgo] = useState('');
	const [outputPath, setOutputpath] = useState('');

	return (
		<>
			<Dialog
				open={props.open}
				setOpen={props.setOpen}
				title="Encrypt a file"
				description="Configure your encryption settings. Leave the output file blank for the default."
				loading={encryptFile.isLoading}
				ctaLabel="Encrypt"
				ctaAction={() => {
					const algorithm = getCryptoSettings(encryptionAlgo, hashingAlgo)[0];
					const output = outputPath !== '' ? outputPath : null;
					props.setOpen(false);

					location_id &&
						object_id &&
						encryptFile.mutate(
							{
								algorithm,
								key_uuid: key,
								location_id,
								object_id,
								metadata,
								preview_media: previewMedia,
								output_path: output
							},
							{
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
							}
						);
				}}
			>
				<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
					<div className="flex flex-col">
						<span className="text-xs font-bold">Key</span>
						<Select
							className="mt-2"
							value={key}
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

				<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
					<div className="flex flex-col">
						<span className="text-xs font-bold">Encryption</span>
						<Select className="mt-2" value={encryptionAlgo} onChange={(e) => setEncryptionAlgo(e)}>
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
							value={hashingAlgo}
						>
							<SelectOption value="Argon2id-s">Argon2id (standard)</SelectOption>
							<SelectOption value="Argon2id-h">Argon2id (hardened)</SelectOption>
							<SelectOption value="Argon2id-p">Argon2id (paranoid)</SelectOption>
							<SelectOption value="BalloonBlake3-s">Blake3-Balloon (standard)</SelectOption>
							<SelectOption value="BalloonBlake3-h">Blake3-Balloon (hardened)</SelectOption>
							<SelectOption value="BalloonBlake3-p">Blake3-Balloon (paranoid)</SelectOption>
						</Select>
					</div>
				</div>

				<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
					<div className="flex">
						<span className="text-sm font-bold mr-3 ml-0.5 mt-0.5">Metadata</span>
						<Checkbox checked={metadata} onChange={(e) => setMetadata(e.target.checked)} />
					</div>
					<div className="flex">
						<span className="text-sm font-bold mr-3 ml-0.5 mt-0.5">Preview Media</span>
						<Checkbox checked={previewMedia} onChange={(e) => setPreviewMedia(e.target.checked)} />
					</div>
				</div>
			</Dialog>
		</>
	);
};
