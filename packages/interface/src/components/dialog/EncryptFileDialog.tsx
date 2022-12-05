import { StoredKey, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, Dialog, Select, SelectOption } from '@sd/ui';
import { save } from '@tauri-apps/api/dialog';
import { useMemo, useState } from 'react';

import { getCryptoSettings } from '../../screens/settings/library/KeysSetting';
import { Checkbox } from '../primitive/Checkbox';

export const ListOfMountedKeys = (props: { keys: StoredKey[]; mountedUuids: string[] }) => {
	const { keys, mountedUuids } = props;

	const [mountedKeys] = useMemo(
		() => [keys.filter((key) => mountedUuids.includes(key.uuid)) ?? []],
		[keys, mountedUuids]
	);

	return (
		<>
			{[...mountedKeys]?.map((key) => {
				return (
					<SelectOption value={key.uuid}>Key {key.uuid.substring(0, 8).toUpperCase()}</SelectOption>
				);
			})}
		</>
	);
};

interface EncryptDialogProps {
	open: boolean;
	setOpen: (isShowing: boolean) => void;
	location_id: number | null;
	object_id: number | null;
	setShowAlertDialog: (isShowing: boolean) => void;
	setAlertDialogData: (data: { title: string; text: string }) => void;
}

export const EncryptFileDialog = (props: EncryptDialogProps) => {
	const { location_id, object_id } = props;
	const keys = useLibraryQuery(['keys.list']);
	const mountedUuids = useLibraryQuery(['keys.listMounted'], {
		onSuccess: (data) => {
			if (key !== data[0]) {
				setKey(data[0]);
			}
		}
	});
	const encryptFile = useLibraryMutation('files.encryptFiles');

	// the selected key will be random, we should prioritise the default
	const [key, setKey] = useState(mountedUuids.data !== undefined ? mountedUuids.data[0] : '');

	// decided against react-hook-form, as it doesn't allow us to work with select boxes and such
	const [metadata, setMetadata] = useState(false);
	const [previewMedia, setPreviewMedia] = useState(false);
	const [encryptionAlgo, setEncryptionAlgo] = useState('XChaCha20Poly1305');
	const [hashingAlgo, setHashingAlgo] = useState('Argon2id-s');
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
					const [algorithm, hashingAlgorithm] = getCryptoSettings(encryptionAlgo, hashingAlgo);
					const output = outputPath !== '' ? outputPath : undefined; // need to add functionality for this in rust
					props.setOpen(false);

					location_id &&
						object_id &&
						encryptFile.mutate(
							{
								algorithm,
								hashing_algorithm: hashingAlgorithm,
								key_uuid: key,
								location_id,
								object_id,
								metadata,
								preview_media: previewMedia
							},
							{
								onSuccess: () => {
									props.setAlertDialogData({
										title: 'Success',
										text: 'The encryption job has started successfully. You may track the progress in the job overview panel.'
									});
								},
								onError: () => {
									props.setAlertDialogData({
										title: 'Error',
										text: 'The encryption job failed to start.'
									});
								}
							}
						);

					props.setShowAlertDialog(true);
				}}
			>
				<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
					<div className="flex flex-col">
						<span className="text-xs font-bold">Mounted Keys</span>
						<Select className="mt-2" value={key} onChange={(e) => setKey(e)}>
							{/* this only returns MOUNTED keys. we could include unmounted keys, but then we'd have to prompt the user to mount them too */}
							{keys.data && mountedUuids.data && (
								<ListOfMountedKeys keys={keys.data} mountedUuids={mountedUuids.data} />
							)}
						</Select>
					</div>
					<div className="flex flex-col">
						<span className="text-xs font-bold">Output file</span>

						<Button
							size="sm"
							variant={outputPath !== '' ? 'accent' : 'gray'}
							className="h-[23px] mt-2"
							onClick={() => {
								// not platform-safe, probably will break on web but `platform` doesn't have a save dialog option
								save()?.then((result) => {
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
						<Select className="mt-2" value={hashingAlgo} onChange={(e) => setHashingAlgo(e)}>
							<SelectOption value="Argon2id-s">Argon2id (standard)</SelectOption>
							<SelectOption value="Argon2id-h">Argon2id (hardened)</SelectOption>
							<SelectOption value="Argon2id-p">Argon2id (paranoid)</SelectOption>
						</Select>
					</div>
				</div>

				<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
					<div className="flex flex-col">
						<span className="text-xs font-bold">Metadata</span>
						<Checkbox checked={metadata} onChange={(e) => setMetadata(e.target.checked)} />
					</div>
					<div className="flex flex-col">
						<span className="text-xs font-bold">Preview Media</span>
						<Checkbox checked={previewMedia} onChange={(e) => setPreviewMedia(e.target.checked)} />
					</div>
				</div>
			</Dialog>
		</>
	);
};
