import clsx from 'clsx';
import { Icon, User } from 'phosphor-react';
import { tw } from '@sd/ui';
import { OperatingSystem } from '../util/Platform';
import { ScreenContainer } from './_Layout';

// TODO: move this to UI, copied from Inspector
const Pill = tw.span`mt-1 inline border border-transparent px-0.5 text-[9px] font-medium shadow shadow-app-shade/5 bg-app-selected rounded text-ink-dull`;

interface DropItemProps {
	name: string;
	icon?: Icon;
	connectionType: 'lan' | 'bluetooth' | 'usb' | 'spacetunnel' | 'p2p';
	receivingNodeOsType: Omit<OperatingSystem, 'unknown'>;
}

function DropItem(props: Partial<DropItemProps>) {
	const Icon = props.icon || User;
	return (
		<div className="overflow-hidden honeycomb-item bg-app-box/60 hover:bg-app-box/50">
			<div className="flex flex-col items-center justify-center w-full h-full">
				<div className="rounded-full w-14 h-14 bg-app-button">
					<Icon className={clsx('w-8 h-8 m-3', !props.name && 'opacity-20')} />
				</div>
				{props.name && <span className="mt-1 text-xs font-medium">{props.name}</span>}
				<div className="flex flex-row space-x-1">
					{props.receivingNodeOsType && <Pill>{props.receivingNodeOsType}</Pill>}
					{props.connectionType && (
						<Pill className="bg-green-500 !text-white uppercase">{props.connectionType}</Pill>
					)}
				</div>
			</div>
		</div>
	);
}

export default function SpacedropScreen() {
	return (
		<ScreenContainer className="honeycomb-outer">
			<div className="honeycomb-container">
				<DropItem name="Jamie's MacBook Pro" receivingNodeOsType="macOs" connectionType="p2p" />
				<DropItem name="Jamie's iPhone" />
				<DropItem name="maxichrome" />
				<DropItem />
				<DropItem />
				<DropItem />
				<DropItem />
				<DropItem />
				<DropItem />
				<DropItem />
				<DropItem />
				<DropItem />
				<DropItem />
				<DropItem />
				<DropItem />
				<DropItem />
				<DropItem />
				<DropItem />
				<DropItem />
			</div>
		</ScreenContainer>
	);
}
