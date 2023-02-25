import cryptoRandomString from 'crypto-random-string';
import { Eye, EyeSlash, Info } from 'phosphor-react';
import { useEffect, useRef, useState } from 'react';
import { Algorithm, useLibraryMutation } from '@sd/client';
import { Button, CategoryHeading, Input, Select, SelectOption, Slider, Switch, tw } from '@sd/ui';
import { Tooltip } from '@sd/ui';
import {
	HashingAlgoSlug,
	getHashingAlgorithmSettings
} from '~/app/:libraryId/settings/library/keys';

const KeyHeading = tw(CategoryHeading)`mb-1`;

export const generatePassword = (length: number) => {
	return cryptoRandomString({ length, type: 'ascii-printable' });
};

export default () => {
	const ref = useRef<HTMLInputElement>(null);
	const [showKey, setShowKey] = useState(false);
	const [librarySync, setLibrarySync] = useState(true);
	const [autoMount, setAutoMount] = useState(false);

	const [sliderValue, setSliderValue] = useState([64]);

	const [key, setKey] = useState('');
	const [encryptionAlgo, setEncryptionAlgo] = useState('XChaCha20Poly1305');
	const [hashingAlgo, setHashingAlgo] = useState<HashingAlgoSlug>('Argon2id-s');

	const createKey = useLibraryMutation('keys.add');
	const CurrentEyeIcon = showKey ? EyeSlash : Eye;

	// this keeps the input focused when switching tabs
	// feel free to replace with something cleaner
	useEffect(() => {
		setTimeout(() => {
			ref.current?.focus();
		});
	}, []);

	return (
		<div className="mb-1 p-3">
			<KeyHeading>Mount key</KeyHeading>
			<div className="flex space-x-2">
				<div className="relative flex grow">
					<Input
						ref={ref}
						value={key}
						onChange={(e) => setKey(e.target.value)}
						autoFocus
						type={showKey ? 'text' : 'password'}
						className="grow !py-0.5"
					/>
					<Button
						onClick={() => setShowKey(!showKey)}
						size="icon"
						className="absolute right-[5px] top-[5px] border-none"
					>
						<CurrentEyeIcon className="h-4 w-4" />
					</Button>
				</div>
			</div>

			<div className="flex flex-row space-x-2">
				<div className="relative mt-2 flex grow">
					<Slider
						value={sliderValue}
						max={128}
						min={8}
						step={4}
						defaultValue={[64]}
						onValueChange={(val) => {
							setSliderValue(val);
							setKey(generatePassword(val[0] ?? 8));
						}}
						onClick={() => {
							setKey(generatePassword(sliderValue[0] ?? 8));
						}}
					/>
				</div>
				<span className="mt-2.5 text-sm font-medium">{sliderValue}</span>
			</div>

			<div className="mt-3 mb-1 flex flex-row items-center">
				<div className="space-x-2">
					<Switch
						className="bg-app-selected"
						size="sm"
						checked={librarySync}
						onCheckedChange={(e) => {
							if (autoMount && !e) setAutoMount(false);
							setLibrarySync(e);
						}}
					/>
				</div>
				<span className="ml-3 text-xs font-medium">Sync with Library</span>
				<Tooltip label="This key will be registered with all devices running your Library">
					<Info className="text-ink-faint ml-1.5 h-4 w-4" />
				</Tooltip>
				<div className="grow" />
				<div className="space-x-2">
					<Switch
						className="bg-app-selected"
						size="sm"
						checked={autoMount}
						onCheckedChange={(e) => {
							if (!librarySync && e) setLibrarySync(true);
							setAutoMount(e);
						}}
					/>
				</div>
				<span className="ml-3 text-xs font-medium">Automount</span>
				<Tooltip label="This key will be automatically mounted every time you unlock the key manager">
					<Info className="text-ink-faint ml-1.5 h-4 w-4" />
				</Tooltip>
			</div>

			<div className="mt-4 mb-3 grid w-full grid-cols-2 gap-4">
				<div className="flex flex-col">
					<span className="text-xs font-bold">Encryption</span>
					<Select className="mt-2" onChange={setEncryptionAlgo} value={encryptionAlgo}>
						<SelectOption value="XChaCha20Poly1305">XChaCha20-Poly1305</SelectOption>
						<SelectOption value="Aes256Gcm">AES-256-GCM</SelectOption>
					</Select>
				</div>
				<div className="flex flex-col">
					<span className="text-xs font-bold">Hashing</span>
					<Select
						className="mt-2"
						onChange={(s) => setHashingAlgo(s as HashingAlgoSlug)}
						value={hashingAlgo}
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
			<Button
				className="mt-2 w-full"
				variant="accent"
				disabled={key === ''}
				onClick={() => {
					setKey('');

					const hashing_algorithm = getHashingAlgorithmSettings(hashingAlgo);

					createKey.mutate({
						algorithm: encryptionAlgo as Algorithm,
						hashing_algorithm,
						key,
						library_sync: librarySync,
						automount: autoMount
					});
				}}
			>
				Mount Key
			</Button>
		</div>
	);
};
