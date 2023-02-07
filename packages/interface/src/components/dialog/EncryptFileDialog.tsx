import { Algorithm, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, Dialog, Select, SelectOption, UseDialogProps, useDialog } from '@sd/ui';
import { CheckBox, useZodForm, z } from '@sd/ui/src/forms';
import { getHashingAlgorithmString } from '~/screens/settings/library/KeysSetting';
import { usePlatform } from '~/util/Platform';
import { showAlertDialog } from '~/util/dialog';
import { SelectOptionKeyList } from '../key/KeyList';

interface EncryptDialogProps extends UseDialogProps {
	location_id: number;
	path_id: number;
}

const schema = z.object({
	key: z.string(),
	encryptionAlgo: z.string(),
	hashingAlgo: z.string(),
	metadata: z.boolean(),
	previewMedia: z.boolean(),
	outputPath: z.string()
});

export const EncryptFileDialog = ({ ...props }: EncryptDialogProps) => {
	const dialog = useDialog(props);
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
			showAlertDialog({
				title: 'Success',
				value:
					'The encryption job has started successfully. You may track the progress in the job overview panel.',
				inputBox: false,
				description: ''
			});
		},
		onError: () => {
			showAlertDialog({
				title: 'Error',
				value: 'The encryption job failed to start.',
				inputBox: false,
				description: ''
			});
		}
	});

	const form = useZodForm({
		defaultValues: { encryptionAlgo: 'XChaCha20Poly1305', outputPath: '' },
		schema
	});

	const onSubmit = form.handleSubmit((data) =>
		encryptFile.mutateAsync({
			algorithm: data.encryptionAlgo as Algorithm,
			key_uuid: data.key,
			location_id: props.location_id,
			path_id: props.path_id,
			metadata: data.metadata,
			preview_media: data.previewMedia,
			output_path: data.outputPath || null
		})
	);

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			dialog={dialog}
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
								showAlertDialog({
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
						value={form.watch('hashingAlgo')}
						disabled
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
					<CheckBox {...form.register('metadata')} />
				</div>
				<div className="flex">
					<span className="text-sm font-bold mr-3 ml-0.5 mt-0.5">Preview Media</span>
					<CheckBox {...form.register('previewMedia')} />
				</div>
			</div>
		</Dialog>
	);
};
