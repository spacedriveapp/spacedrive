import { InformationCircleIcon } from '@heroicons/react/24/outline';
import {
	EllipsisVerticalIcon,
	EyeIcon,
	EyeSlashIcon,
	KeyIcon,
	LockClosedIcon,
	LockOpenIcon,
	PlusIcon,
	TrashIcon,
	XMarkIcon
} from '@heroicons/react/24/solid';
import { Button, CategoryHeading, Input, Select, SelectOption } from '@sd/ui';
import clsx from 'clsx';
import { Eject, EjectSimple, Plus } from 'phosphor-react';
import { useEffect, useRef, useState } from 'react';

import { Toggle } from '../primitive';
import { DefaultProps } from '../primitive/types';
import { Tooltip } from '../tooltip/Tooltip';
import { Key } from './Key';

export function KeyMounter() {
	const ref = useRef<HTMLInputElement>(null);

	const [showKey, setShowKey] = useState(false);
	const [toggle, setToggle] = useState(false);

	const [key, setKey] = useState('');
	const [encryptionAlgo, setEncryptionAlgo] = useState('XChaCha20Poly1305');
	const [hashingAlgo, setHashingAlgo] = useState('Argon2id');

	const CurrentEyeIcon = showKey ? EyeSlashIcon : EyeIcon;

	// this keeps the input focused when switching tabs
	// feel free to replace with something cleaner
	useEffect(() => {
		setTimeout(() => {
			ref.current?.focus();
		});
	}, []);

	return (
		<div className="p-3 pt-3 mb-1">
			<CategoryHeading>Mount key</CategoryHeading>
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
						noBorder
						noPadding
						className="absolute right-[5px] top-[5px]"
					>
						<CurrentEyeIcon className="w-4 h-4" />
					</Button>
				</div>
			</div>

			<div className="flex flex-row items-center mt-3 mb-1">
				<Toggle className="dark:bg-gray-400/30" size="sm" value={toggle} onChange={setToggle} />
				<span className="ml-3 mt-[1px] font-medium text-xs">Sync with Library</span>
				<Tooltip label="This key will be mounted on all devices running your Library">
					<InformationCircleIcon className="w-4 h-4 ml-1.5 text-gray-400" />
				</Tooltip>
			</div>

			<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
				<div className="flex flex-col">
					<span className="text-xs font-bold">Encryption</span>
					<Select className="mt-2" onChange={setEncryptionAlgo} value={encryptionAlgo}>
						<SelectOption value="XChaCha20Poly1305">XChaCha20Poly1305</SelectOption>
						<SelectOption value="Aes256Gcm">Aes256Gcm</SelectOption>
					</Select>
				</div>
				<div className="flex flex-col">
					<span className="text-xs font-bold">Hashing</span>
					<Select className="mt-2" onChange={setHashingAlgo} value={hashingAlgo}>
						<SelectOption value="Argon2id">Argon2id</SelectOption>
					</Select>
				</div>
			</div>
			<p className="pt-1.5 ml-0.5 text-[8pt] leading-snug text-gray-300 opacity-50 w-[90%]">
				Files encrypted with this key will be revealed and decrypted on the fly.
			</p>
			<Button className="w-full mt-2" variant="primary">
				Mount Key
			</Button>
		</div>
	);
}
