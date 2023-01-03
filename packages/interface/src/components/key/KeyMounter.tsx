import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, CategoryHeading, Input, Select, SelectOption, Switch, cva, tw } from '@sd/ui';
import cryptoRandomString from 'crypto-random-string';
import { Eye, EyeSlash, Info } from 'phosphor-react';
import { useEffect, useRef, useState } from 'react';

import { getCryptoSettings } from '../../screens/settings/library/KeysSetting';
import Slider from '../primitive/Slider';
import { Tooltip } from '../tooltip/Tooltip';

const KeyHeading = tw(CategoryHeading)`mb-1`;

const PasswordCharset =
	'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()_+-={}[]:"\';<>?,./\\|`~';

const GeneratePassword = (length: number) => {
	return cryptoRandomString({ length, characters: PasswordCharset });
};

export function KeyMounter() {
	const ref = useRef<HTMLInputElement>(null);
	const [showKey, setShowKey] = useState(false);
	const [librarySync, setLibrarySync] = useState(true);
	const [autoMount, setAutoMount] = useState(false);

	const [sliderValue, setSliderValue] = useState([64]);

	const [key, setKey] = useState('');
	const [encryptionAlgo, setEncryptionAlgo] = useState('XChaCha20Poly1305');
	const [hashingAlgo, setHashingAlgo] = useState('Argon2id-s');

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
		<div className="p-3 pt-3 mb-1">
			<KeyHeading>Mount key</KeyHeading>
			<div className="flex space-x-2">
				<div className="relative flex flex-grow">
					<Input
						ref={ref}
						value={key}
						onChange={(e) => setKey(e.target.value)}
						autoFocus
						type={showKey ? 'text' : 'password'}
						className="flex-grow !py-0.5"
					/>
					<Button
						onClick={() => setShowKey(!showKey)}
						size="icon"
						className="border-none absolute right-[5px] top-[5px]"
					>
						<CurrentEyeIcon className="w-4 h-4" />
					</Button>
				</div>
			</div>

			<div className="flex flex-row space-x-2">
				<div className="relative flex flex-grow mt-2">
					<Slider
						value={sliderValue}
						max={128}
						min={8}
						step={4}
						defaultValue={[64]}
						onValueChange={(e) => {
							setSliderValue(e);
							setKey(GeneratePassword(e[0]));
						}}
						onClick={() => {
							setKey(GeneratePassword(sliderValue[0]));
						}}
					/>
				</div>
				<span className="text-sm mt-2.5 font-medium">{sliderValue}</span>
			</div>

			<div className="flex flex-row items-center mt-3 mb-1">
				<div className="space-x-2">
					<Switch
						className="bg-app-selected"
						size="sm"
						checked={librarySync}
						onCheckedChange={(e) => {
							if (autoMount && e) setAutoMount(false);
							setLibrarySync(e);
						}}
					/>
				</div>
				<span className="ml-3 text-xs font-medium">Sync with Library</span>
				<Tooltip label="This key will be registered with all devices running your Library">
					<Info className="w-4 h-4 ml-1.5 text-ink-faint" />
				</Tooltip>
				<div className="flex-grow" />
				<div className="space-x-2">
					<Switch
						className="bg-app-selected"
						size="sm"
						checked={autoMount}
						onCheckedChange={(e) => {
							if (librarySync && e) setLibrarySync(false);
							setAutoMount(e);
						}}
					/>
				</div>
				<span className="ml-3 text-xs font-medium">Automount</span>
				<Tooltip label="This key will be automatically mounted every time you unlock the key manager">
					<Info className="w-4 h-4 ml-1.5 text-ink-faint" />
				</Tooltip>
			</div>

			<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
				<div className="flex flex-col">
					<span className="text-xs font-bold">Encryption</span>
					<Select className="mt-2" onChange={setEncryptionAlgo} value={encryptionAlgo}>
						<SelectOption value="XChaCha20Poly1305">XChaCha20-Poly1305</SelectOption>
						<SelectOption value="Aes256Gcm">AES-256-GCM</SelectOption>
					</Select>
				</div>
				<div className="flex flex-col">
					<span className="text-xs font-bold">Hashing</span>
					<Select className="mt-2" onChange={setHashingAlgo} value={hashingAlgo}>
						<SelectOption value="Argon2id-s">Argon2id (standard)</SelectOption>
						<SelectOption value="Argon2id-h">Argon2id (hardened)</SelectOption>
						<SelectOption value="Argon2id-p">Argon2id (paranoid)</SelectOption>
					</Select>
				</div>
			</div>
			<Button
				className="w-full mt-2"
				variant="accent"
				disabled={key === ''}
				onClick={() => {
					setKey('');

					const [algorithm, hashing_algorithm] = getCryptoSettings(encryptionAlgo, hashingAlgo);

					createKey.mutate({
						algorithm,
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
}
