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
import { Button } from '@sd/ui';
import { Loader } from '@sd/ui';
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
		<div className="border-app-divider bg-app w-full rounded-md border">
			<div className="flex flex-row items-center px-4 pt-2 pb-2">
				<DotsSixVertical weight="bold" className="mr-3 opacity-30" />
				{props.type === 'phone' && <DeviceMobileCamera weight="fill" size={20} className="mr-2" />}
				{props.type === 'laptop' && <Laptop weight="fill" size={20} className="mr-2" />}
				{props.type === 'desktop' && <Desktop weight="fill" size={20} className="mr-2" />}
				{props.type === 'server' && <Cloud weight="fill" size={20} className="mr-2" />}
				<h3 className="text-md font-semibold">{props.name || 'Unnamed Device'}</h3>
				<div className="mt-0.5 flex flex-row space-x-1.5">
					<span className="text-type-faint -mt-0.5 ml-3 flex h-[19px] flex-row rounded py-0.5 px-1.5 text-[10px] font-semibold">
						<Lock weight="bold" className="m-[1px] mr-1 -ml-0.5 h-3 w-3" />
						P2P
					</span>
				</div>
				<span className="ml-2 py-0.5 px-1.5 text-sm font-semibold  ">{props.size}</span>
				<div className="flex flex-grow" />
				{props.runningJob && (
					<div className="ml-5 flex flex-row rounded-md bg-opacity-50 ">
						<Loader />
						<div className="flex flex-col p-2">
							<span className="text-tiny mb-[2px] -mt-1 truncate">{props.runningJob.task}...</span>
							<ProgressBar value={props.runningJob?.amount} total={100} />
						</div>
					</div>
				)}
				<div className="ml-3 flex flex-row space-x-1">
					<Tooltip label="Encrypt">
						<Button className="!p-1 ">
							<Key weight="bold" className="h-5 w-5" />
						</Button>
					</Tooltip>
					<Tooltip label="Settings">
						<Button className="!p-1 ">
							<Gear weight="bold" className="h-5 w-5" />
						</Button>
					</Tooltip>
				</div>
			</div>
			<div className="mt-3 px-4 pb-3">
				{props.locations.length === 0 && (
					<div className="my-5 w-full text-center">No locations</div>
				)}
			</div>
		</div>
	);
}
