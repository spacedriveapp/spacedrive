import { InformationCircleIcon } from '@heroicons/react/24/outline';
import {
	EyeIcon,
	EyeSlashIcon,
	KeyIcon,
	LockClosedIcon,
	LockOpenIcon,
	PlusIcon,
	TrashIcon,
	XMarkIcon
} from '@heroicons/react/24/solid';
import { Button, Input } from '@sd/ui';
import clsx from 'clsx';
import { Eject, EjectSimple, Plus } from 'phosphor-react';
import { useState } from 'react';

import { Toggle } from '../primitive';
import { DefaultProps } from '../primitive/types';
import { Tooltip } from '../tooltip/Tooltip';

export type KeyManagerProps = DefaultProps;

interface FakeKey {
	id: string;
	name: string;
	mounted?: boolean;
	locked?: boolean;
	stats?: {
		objectCount?: number;
		containerCount?: number;
	};
	// Nodes this key is mounted on
	nodes?: string[]; // will be node object
}

const Heading: React.FC<{ children: React.ReactNode }> = ({ children }) => (
	<div className="mt-1 mb-1 text-xs font-semibold text-gray-300">{children}</div>
);

const Key: React.FC<{ data: FakeKey; index: number }> = ({ data, index }) => {
	const odd = (index || 0) % 2 === 0;

	return (
		<div
			className={clsx(
				'flex items-center justify-between px-2 py-1.5 shadow-gray-900/20 text-sm text-gray-300 bg-gray-500/30 shadow-lg border-gray-500 rounded-lg'
				// !odd && 'bg-opacity-10'
			)}
		>
			<div className="flex items-center">
				<KeyIcon
					className={clsx(
						'w-5 h-5 ml-1 mr-3',
						data.mounted
							? data.locked
								? 'text-primary-600'
								: 'text-primary-600'
							: 'text-gray-400/80'
					)}
				/>
				<div className="flex flex-col ">
					<div className="flex flex-row items-center">
						<div className="font-semibold">{data.name}</div>
						{data.mounted && (
							<div className="inline ml-2 px-1 text-[8pt] font-medium text-gray-300 bg-gray-500 rounded">
								{data.nodes?.length || 0 > 0 ? `${data.nodes?.length || 0} nodes` : 'This node'}
							</div>
						)}
					</div>
					{/* <div className="text-xs text-gray-300 opacity-30">#{data.id}</div> */}
					{/* {data.stats && (
						<div className="flex flex-row space-x-3">
							{data.stats.objectCount && (
								<div className="text-[8pt] text-gray-300 opacity-30">
									{data.stats.objectCount} Objects
								</div>
							)}
							{data.stats.containerCount && (
								<div className="text-[8pt] text-gray-300 opacity-30">
									{data.stats.containerCount} Containers
								</div>
							)}
						</div>
					)} */}
				</div>
			</div>
			<div className="space-x-1">
				{data.mounted ? (
					<>
						<Tooltip label="Browse files">
							<Button noPadding>
								<EyeIcon className="w-4 h-4 text-gray-400" />
							</Button>
						</Tooltip>

						{data.locked ? (
							<Tooltip label="Unlock key">
								<Button noPadding>
									<LockClosedIcon className="w-4 h-4 text-gray-400" />
								</Button>
							</Tooltip>
						) : (
							<Tooltip label="Lock key">
								<Button noPadding>
									<LockOpenIcon className="w-4 h-4 text-gray-400" />
								</Button>
							</Tooltip>
						)}
					</>
				) : (
					<Tooltip label="Dismount key">
						<Button noPadding>
							<XMarkIcon className="w-4 h-4 text-gray-400" />
						</Button>
					</Tooltip>
				)}
			</div>
		</div>
	);
};

export function KeyManager(props: KeyManagerProps) {
	const [showKey, setShowKey] = useState(false);
	const [toggle, setToggle] = useState(false);

	const CurrentEyeIcon = showKey ? EyeSlashIcon : EyeIcon;

	return (
		<div className="flex flex-col h-full">
			<div className="p-3 pt-3">
				<Heading>Mount key</Heading>
				<div className="flex space-x-2">
					<div className="relative flex flex-grow">
						<Input autoFocus type={showKey ? 'text' : 'password'} className="flex-grow !py-0.5" />
						<Button
							onClick={() => setShowKey(!showKey)}
							noBorder
							noPadding
							className="absolute right-[5px] top-[5px]"
						>
							<CurrentEyeIcon className="w-4 h-4" />
						</Button>
					</div>
					<Tooltip className="flex" label="Mount key">
						<Button variant="gray" noPadding>
							<Plus weight="fill" className="w-4 h-4 mx-1" />
						</Button>
					</Tooltip>
				</div>
				<div className="flex flex-row items-center mt-3 mb-1">
					<Toggle className="dark:bg-gray-400/30" size="sm" value={toggle} onChange={setToggle} />
					<span className="ml-3 mt-[1px] font-medium text-xs">Sync with Library</span>
					<Tooltip label="This key will be mounted on all devices running your Library">
						<InformationCircleIcon className="w-4 h-4 ml-1.5 text-gray-400" />
					</Tooltip>
				</div>
				{/* <p className="pt-1.5 ml-0.5 text-[8pt] leading-snug text-gray-300 opacity-50 w-[90%]">
					Files encrypted with this key will be revealed and decrypted on the fly.
				</p> */}
			</div>
			<hr className="border-gray-500" />
			<div className="p-3 custom-scroll overlay-scroll">
				<div className="">
					<Heading>Mounted keys</Heading>
					<div className="pt-1 space-y-1.5">
						<Key
							index={0}
							data={{
								id: 'af5570f5a1810b7a',
								name: 'OBS Recordings',
								mounted: true,

								nodes: ['node1', 'node2'],
								stats: { objectCount: 235, containerCount: 2 }
							}}
						/>
						<Key
							index={1}
							data={{
								id: 'af5570f5a1810b7a',
								name: 'Unknown Key',
								locked: true,
								mounted: true,
								stats: { objectCount: 45 }
							}}
						/>
						<Key index={2} data={{ id: '7324695a52da67b1', name: 'Spacedrive Company' }} />
						<Key index={3} data={{ id: 'b02303d68d05a562', name: 'Key 4' }} />
						<Key index={3} data={{ id: 'b02303d68d05a562', name: 'Key 5' }} />
						<Key index={3} data={{ id: 'b02303d68d05a562', name: 'Key 6' }} />
					</div>
				</div>
			</div>
			<div className="flex w-full p-2 bg-gray-600 border-t border-gray-500 rounded-b-md">
				<Button size="sm" variant="gray">
					Unmount All
				</Button>
				<div className="flex-grow" />
				<Button size="sm" variant="gray">
					Close
				</Button>
			</div>
		</div>
	);
}
