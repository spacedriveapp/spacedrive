import { Button, CategoryHeading, Input, Select, SelectOption, Switch, cva, tw } from '@sd/ui';
import { Eye, EyeSlash, Info } from 'phosphor-react';
import { useEffect, useRef, useState } from 'react';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Algorithm, HashingAlgorithm, Params } from '@sd/client';

import { Tooltip } from '../tooltip/Tooltip';

const KeyHeading = tw(CategoryHeading)`mb-1`;

export function KeyMounter() {
	const ref = useRef<HTMLInputElement>(null);

	// we need to call these at least once somewhere
	// if we don't, if a user mounts a key before first viewing the key list, no key will show in the list
	// either call it in here or in the keymanager itself
	const keys = useLibraryQuery(['keys.list']);
	const mounted_uuids = useLibraryQuery(['keys.listMounted']);

	const [showKey, setShowKey] = useState(false);
	const [toggle, setToggle] = useState(true);

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

			<div className="flex flex-row items-center mt-3 mb-1">
				<div className="space-x-2">
					<Switch
						className="bg-app-selected"
						size="sm"
						checked={toggle}
						onCheckedChange={setToggle}
					/>
				</div>
				<span className="ml-3 text-xs font-medium">Sync with Library</span>
				<Tooltip label="This key will be mounted on all devices running your Library">
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
			<p className="pt-1.5 ml-0.5 text-[8pt] leading-snug text-ink-faint w-[90%]">
				Files encrypted with this key will be revealed and decrypted on the fly.
			</p>
			<Button className="w-full mt-2" variant="accent" onClick={() => {
				let algorithm = encryptionAlgo as Algorithm;
				let hashing_algorithm: HashingAlgorithm = { Argon2id: "Standard" };

				switch(hashingAlgo) {
					case "Argon2id-s":
						hashing_algorithm = { Argon2id: "Standard" as Params };
						break;
					case "Argon2id-h":
						hashing_algorithm = { Argon2id: "Hardened" as Params };
						break;
					case "Argon2id-p":
						hashing_algorithm = { Argon2id: "Paranoid" as Params };
						break;
				}

				createKey.mutate({algorithm, hashing_algorithm, key });
				setKey("");
			}
			}>
				Mount Key
			</Button>
		</div>
	);
}
