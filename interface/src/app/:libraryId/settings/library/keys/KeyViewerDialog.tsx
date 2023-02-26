import { Buffer } from 'buffer';
import { Clipboard } from 'phosphor-react';
import { useState } from 'react';
import { slugFromHashingAlgo, useLibraryQuery } from '@sd/client';
import { Button, Dialog, Input, Select, SelectOption, UseDialogProps, useDialog } from '@sd/ui';
import { useZodForm } from '@sd/ui/src/forms';
import { KeyListSelectOptions } from '~/app/:libraryId/KeyManager/List';

export const KeyUpdater = (props: {
	uuid: string;
	setKey: (value: string) => void;
	setEncryptionAlgo: (value: string) => void;
	setHashingAlgo: (value: string) => void;
	setContentSalt: (value: string) => void;
}) => {
	useLibraryQuery(['keys.getKey', props.uuid], {
		onSuccess: (data) => {
			props.setKey(data);
		}
	});

	const keys = useLibraryQuery(['keys.list']);

	const key = keys.data?.find((key) => key.uuid == props.uuid);

	if (key) {
		props.setEncryptionAlgo(key?.algorithm);
		props.setHashingAlgo(slugFromHashingAlgo(key?.hashing_algorithm));
		props.setContentSalt(Buffer.from(key.content_salt).toString('hex'));
	}

	return <></>;
};

export default (props: UseDialogProps) => {
	const form = useZodForm();
	const dialog = useDialog(props);

	const keys = useLibraryQuery(['keys.list'], {
		onSuccess: (data) => {
			if (key === '' && data.length !== 0) {
				setKey(data[0]?.uuid ?? '');
			}
		}
	});

	const [key, setKey] = useState('');
	const [keyValue, setKeyValue] = useState('');
	const [contentSalt, setContentSalt] = useState('');
	const [encryptionAlgo, setEncryptionAlgo] = useState('');
	const [hashingAlgo, setHashingAlgo] = useState('');

	return (
		<Dialog
			form={form}
			onSubmit={form.handleSubmit(() => {})}
			dialog={dialog}
			title="View Key Values"
			description="Here you can view the values of your keys."
			ctaLabel="Done"
		>
			<KeyUpdater
				uuid={key}
				setKey={setKeyValue}
				setEncryptionAlgo={setEncryptionAlgo}
				setHashingAlgo={setHashingAlgo}
				setContentSalt={setContentSalt}
			/>

			<div className="mt-4 mb-3 grid w-full gap-4">
				<div className="flex flex-col">
					<span className="text-xs font-bold">Key</span>
					<Select
						className="mt-2 grow"
						value={key}
						onChange={(e) => {
							setKey(e);
						}}
					>
						{keys.data && <KeyListSelectOptions keys={keys.data.map((key) => key.uuid)} />}
					</Select>
				</div>
			</div>
			<div className="mt-4 mb-3 grid w-full grid-cols-2 gap-4">
				<div className="flex flex-col">
					<span className="text-xs font-bold">Encryption</span>
					<Select
						className="mt-2 w-[150px] text-gray-300"
						value={encryptionAlgo}
						disabled
						onChange={() => {}}
					>
						<SelectOption value="XChaCha20Poly1305">XChaCha20-Poly1305</SelectOption>
						<SelectOption value="Aes256Gcm">AES-256-GCM</SelectOption>
					</Select>
				</div>
				<div className="flex flex-col">
					<span className="text-xs font-bold">Hashing</span>
					<Select className="mt-2 text-gray-300" value={hashingAlgo} disabled onChange={() => {}}>
						<SelectOption value="Argon2id-s">Argon2id (standard)</SelectOption>
						<SelectOption value="Argon2id-h">Argon2id (hardened)</SelectOption>
						<SelectOption value="Argon2id-p">Argon2id (paranoid)</SelectOption>
						<SelectOption value="BalloonBlake3-s">BLAKE3-Balloon (standard)</SelectOption>
						<SelectOption value="BalloonBlake3-h">BLAKE3-Balloon (hardened)</SelectOption>
						<SelectOption value="BalloonBlake3-p">BLAKE3-Balloon (paranoid)</SelectOption>
					</Select>
				</div>
			</div>
			<div className="mt-4 mb-3 grid w-full gap-4">
				<div className="flex flex-col">
					<span className="mb-2 text-xs font-bold">Content Salt (hex)</span>
					<div className="relative flex grow">
						<Input value={contentSalt} disabled className="grow !py-0.5" />
						<Button
							type="button"
							onClick={() => {
								navigator.clipboard.writeText(contentSalt);
							}}
							size="icon"
							className="absolute right-[5px] top-[5px] border-none"
						>
							<Clipboard className="h-4 w-4" />
						</Button>
					</div>
				</div>
			</div>
			<div className="mt-4 mb-3 grid w-full gap-4">
				<div className="flex flex-col">
					<span className="mb-2 text-xs font-bold">Key Value</span>
					<div className="relative flex grow">
						<Input value={keyValue} disabled className="grow !py-0.5" />
						<Button
							type="button"
							onClick={() => {
								navigator.clipboard.writeText(keyValue);
							}}
							size="icon"
							className="absolute right-[5px] top-[5px] border-none"
						>
							<Clipboard className="h-4 w-4" />
						</Button>
					</div>
				</div>
			</div>
		</Dialog>
	);
};
