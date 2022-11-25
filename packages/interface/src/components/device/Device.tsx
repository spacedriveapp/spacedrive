import { Button } from '@sd/ui';
import { Loader } from '@sd/ui';
import {
	Cloud,
	Desktop,
	DeviceMobileCamera,
	DotsSixVertical,
	Gear,
	Key,
	Laptop,
	Lock
} from 'phosphor-react';
import { useState } from 'react';

import FileItem from '../explorer/FileItem';
import ProgressBar from '../primitive/ProgressBar';
import { Tooltip } from '../tooltip/Tooltip';

export interface DeviceProps {
	name: string;
	size: string;
	type: 'laptop' | 'desktop' | 'phone' | 'server';
	locations: { name: string; folder?: boolean; format?: string; icon?: string }[];
	runningJob?: { amount: number; task: string };
}

export function Device(props: DeviceProps) {
	const [selectedFile, setSelectedFile] = useState<null | string>(null);

	function handleSelect(key: string) {
		if (selectedFile === key) setSelectedFile(null);
		else setSelectedFile(key);
	}

	return (
		<div className="w-full border rounded-md border-app-divider bg-app">
			<div className="flex flex-row items-center px-4 pt-2 pb-2">
				<DotsSixVertical weight="bold" className="mr-3 opacity-30" />
				{props.type === 'phone' && <DeviceMobileCamera weight="fill" size={20} className="mr-2" />}
				{props.type === 'laptop' && <Laptop weight="fill" size={20} className="mr-2" />}
				{props.type === 'desktop' && <Desktop weight="fill" size={20} className="mr-2" />}
				{props.type === 'server' && <Cloud weight="fill" size={20} className="mr-2" />}
				<h3 className="font-semibold text-md">{props.name || 'Unnamed Device'}</h3>
				<div className="flex flex-row space-x-1.5 mt-0.5">
					<span className="font-semibold flex flex-row h-[19px] -mt-0.5 ml-3 py-0.5 px-1.5 text-[10px] rounded text-type-faint">
						<Lock weight="bold" className="w-3 h-3 mr-1 -ml-0.5 m-[1px]" />
						P2P
					</span>
				</div>
				<span className="font-semibold py-0.5 px-1.5 text-sm ml-2  ">{props.size}</span>
				<div className="flex flex-grow" />
				{props.runningJob && (
					<div className="flex flex-row ml-5 bg-opacity-50 rounded-md ">
						<Loader />
						<div className="flex flex-col p-2">
							<span className="mb-[2px] -mt-1 truncate text-tiny">{props.runningJob.task}...</span>
							<ProgressBar value={props.runningJob?.amount} total={100} />
						</div>
					</div>
				)}
				<div className="flex flex-row ml-3 space-x-1">
					<Tooltip label="Encrypt">
						<Button className="!p-1 ">
							<Key weight="bold" className="w-5 h-5" />
						</Button>
					</Tooltip>
					<Tooltip label="Settings">
						<Button className="!p-1 ">
							<Gear weight="bold" className="w-5 h-5" />
						</Button>
					</Tooltip>
				</div>
			</div>
			<div className="px-4 pb-3 mt-3">
				{props.locations.length === 0 && (
					<div className="w-full my-5 text-center">No locations</div>
				)}
			</div>
		</div>
	);
}
