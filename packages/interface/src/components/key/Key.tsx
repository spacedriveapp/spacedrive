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
import { Button, Input, Select, SelectOption } from '@sd/ui';
import clsx from 'clsx';
import { Eject, EjectSimple, Plus } from 'phosphor-react';
import { useState } from 'react';

import { Toggle } from '../primitive';
import { DefaultProps } from '../primitive/types';
import { Tooltip } from '../tooltip/Tooltip';

export type KeyManagerProps = DefaultProps;

// TODO: Replace this with Prisma type when integrating with backend
export interface Key {
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

export const Key: React.FC<{ data: Key; index: number }> = ({ data, index }) => {
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
					{data.stats ? (
						<div className="flex flex-row mt-[1px] space-x-3">
							{data.stats.objectCount && (
								<div className="text-[8pt] font-medium text-gray-200 opacity-30">
									{data.stats.objectCount} Objects
								</div>
							)}
							{data.stats.containerCount && (
								<div className="text-[8pt] font-medium text-gray-200 opacity-30">
									{data.stats.containerCount} Containers
								</div>
							)}
						</div>
					) : (
						!data.mounted && (
							<div className="text-[8pt] font-medium text-gray-200 opacity-30">Key not mounted</div>
						)
					)}
				</div>
			</div>
			<div className="space-x-1">
				{data.mounted && (
					<Tooltip label="Browse files">
						<Button noPadding>
							<EyeIcon className="w-4 h-4 text-gray-400" />
						</Button>
					</Tooltip>
				)}
				<Button noPadding>
					<EllipsisVerticalIcon className="w-4 h-4 text-gray-400" />
				</Button>
			</div>
		</div>
	);
};
