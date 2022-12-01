import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, Dialog, Input, Select, SelectOption } from '@sd/ui';
import { save } from '@tauri-apps/api/dialog';
import { Eye, EyeSlash } from 'phosphor-react';
import { ReactNode, useMemo, useState } from 'react';
import { SubmitHandler, useForm } from 'react-hook-form';

import { getCryptoSettings } from '../../screens/settings/library/KeysSetting';

export const ListOfMountedKeys = () => {
	// enumerating keys this way allows us to have more information, so we can prioritise default keys/prompt the user to mount a key, etc
	const keys = useLibraryQuery(['keys.list']);
	const mounted_uuids = useLibraryQuery(['keys.listMounted']);
	const default_key = useLibraryQuery(['keys.getDefault']);

	const [mountedKeys, unmountedKeys] = useMemo(
		() => [
			keys.data?.filter((key) => mounted_uuids.data?.includes(key.uuid)) ?? [],
			keys.data?.filter((key) => !mounted_uuids.data?.includes(key.uuid)) ?? []
		],
		[keys, mounted_uuids]
	);

	return (
		<>
			{[...mountedKeys]?.map((key, index) => {
				return <SelectOption value={key.uuid}>`Key ${index + 1}`</SelectOption>;
			})}
		</>
	);
};

export const EncryptFileDialog = (props: { trigger: ReactNode }) => {
	type FormValues = {
		outputPath: string;
		key: string;
		encryptionAlgo: string;
		hashingAlgo: string;
	};

	const { register, handleSubmit, getValues, setValue } = useForm<FormValues>({
		defaultValues: {
			outputPath: '',
			key: '',
			encryptionAlgo: 'XChaCha20Poly1305',
			hashingAlgo: 'Argon2id-s'
		}
	});

	const onSubmit: SubmitHandler<FormValues> = (data) => {
		const [algorithm, hashing_algorithm] = getCryptoSettings(data.encryptionAlgo, data.hashingAlgo);

		// changeMasterPassword.mutate(
		// 	{ algorithm, hashing_algorithm, password: data.masterPassword },
		// 	{
		// 		onSuccess: (sk) => {
		// 			setSecretKey(sk);

		// 			setShowEncryptFileDialog(false);
		// 		},
		// 		onError: () => {
		// 			// this should never really happen
		// 			alert('There was an error while changing your master password.');
		// 		}
		// 	}
		// );
	};

	const [showEncryptFileDialog, setShowEncryptFileDialog] = useState(false);
	const encryptFile = useLibraryMutation('files.encryptFiles');
	const { trigger } = props;

	return (
		// this also needs options for metadata/preview media inclusion
		<>
			<form onSubmit={handleSubmit(onSubmit)}>
				<Dialog
					open={showEncryptFileDialog}
					setOpen={setShowEncryptFileDialog}
					title="Change Master Password"
					description="Select a new master password for your key manager."
					ctaDanger={true}
					loading={encryptFile.isLoading}
					ctaLabel="Change"
					trigger={trigger}
				>
					<Button
						size="sm"
						variant={getValues('outputPath') !== '' ? 'accent' : 'gray'}
						className="mr-2"
						onClick={() => {
							// not platform-safe, probably will break on web but `platform` doesn't have a save dialog option
							save()?.then((result) => {
								if (result) setValue('outputPath', result as string);
							});
						}}
					>
						Backup
					</Button>

					<div className="flex flex-col">
						<span className="text-xs font-bold">Keys</span>
						<Select className="mt-2" value={getValues('key')} onChange={(e) => setValue('key', e)}>
							{/* need a function to iterate over and return all MOUNTED here, numbered. we could include unmounted keys, but then we'd have to prompt the user to mount them too */}
							<ListOfMountedKeys />
						</Select>
					</div>

					<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
						<div className="flex flex-col">
							<span className="text-xs font-bold">Encryption</span>
							<Select
								className="mt-2"
								value={getValues('encryptionAlgo')}
								onChange={(e) => setValue('encryptionAlgo', e)}
							>
								<SelectOption value="XChaCha20Poly1305">XChaCha20-Poly1305</SelectOption>
								<SelectOption value="Aes256Gcm">AES-256-GCM</SelectOption>
							</Select>
						</div>
						<div className="flex flex-col">
							<span className="text-xs font-bold">Hashing</span>
							<Select
								className="mt-2"
								value={getValues('hashingAlgo')}
								onChange={(e) => setValue('hashingAlgo', e)}
							>
								<SelectOption value="Argon2id-s">Argon2id (standard)</SelectOption>
								<SelectOption value="Argon2id-h">Argon2id (hardened)</SelectOption>
								<SelectOption value="Argon2id-p">Argon2id (paranoid)</SelectOption>
							</Select>
						</div>
					</div>
				</Dialog>
			</form>
		</>
	);
};
