import GoogleDrive from '@sd/assets/images/GoogleDrive.png';
import Mega from '@sd/assets/images/Mega.png';
import iCloud from '@sd/assets/images/iCloud.png';
import clsx from 'clsx';
import { DeviceMobile, HardDrives, Icon, Laptop, Star, User } from 'phosphor-react';
import { useRef } from 'react';
import { tw } from '@sd/ui';
import { SearchBar } from '../components/explorer/ExplorerTopBar';
import { SubtleButton, SubtleButtonContainer } from '../components/primitive/SubtleButton';
import { OperatingSystem } from '../util/Platform';
import * as PageLayout from './PageLayout';
import classes from './Spacedrop.module.scss';

// TODO: move this to UI, copied from Inspector
const Pill = tw.span`mt-1 inline border border-transparent px-0.5 text-[9px] font-medium shadow shadow-app-shade/5 bg-app-selected rounded text-ink-dull`;

type DropItemProps = {
	// TODO: remove optionals when dummy data is removed (except for icon)
	name?: string;
	connectionType?: 'lan' | 'bluetooth' | 'usb' | 'p2p' | 'cloud';
	receivingNodeOsType?: Omit<OperatingSystem, 'unknown'>;
} & ({ image: string } | { icon?: Icon } | { brandIcon: string });

function DropItem(props: DropItemProps) {
	let icon;
	if ('image' in props) {
		icon = <img className="rounded-full" src={props.image} alt={props.name} />;
	} else if ('brandIcon' in props) {
		let brandIconSrc;
		switch (props.brandIcon) {
			case 'google-drive':
				brandIconSrc = GoogleDrive;
				break;
			case 'icloud':
				brandIconSrc = iCloud;
				break;
			case 'mega':
				brandIconSrc = Mega;
				break;
		}
		if (brandIconSrc) {
			icon = (
				<div className="flex h-full items-center justify-center p-3">
					<img className="rounded-full " src={brandIconSrc} alt={props.name} />
				</div>
			);
		}
	} else {
		//
		const Icon = props.icon || User;
		icon = <Icon className={clsx('m-3 h-8 w-8', !props.name && 'opacity-20')} />;
	}

	return (
		<div
			className={clsx(classes.honeycombItem, 'bg-app-box/20 hover:bg-app-box/50 overflow-hidden')}
		>
			<div className="group relative flex h-full w-full flex-col items-center justify-center ">
				<SubtleButtonContainer className="absolute left-[12px] top-[55px]">
					<SubtleButton icon={Star} />
				</SubtleButtonContainer>
				<div className="bg-app-button h-14 w-14 rounded-full">{icon}</div>
				<SubtleButtonContainer className="absolute right-[12px] top-[55px] rotate-90">
					<SubtleButton />
				</SubtleButtonContainer>
				{props.name && <span className="mt-1 text-xs font-medium">{props.name}</span>}
				<div className="flex flex-row space-x-1">
					{props.receivingNodeOsType && <Pill>{props.receivingNodeOsType}</Pill>}
					{props.connectionType && (
						<Pill
							className={clsx(
								'uppercase !text-white',
								props.connectionType === 'lan' && 'bg-green-500',
								props.connectionType === 'p2p' && 'bg-blue-500'
							)}
						>
							{props.connectionType}
						</Pill>
					)}
				</div>
			</div>
		</div>
	);
}

export default () => {
	const searchRef = useRef<HTMLInputElement>(null);

	return (
		<>
			<PageLayout.DragChildren>
				<div className="flex h-8 w-full flex-row items-center justify-center pt-3">
					<SearchBar className="ml-[13px]" ref={searchRef} />
					{/* <Button variant="outline">Add</Button> */}
				</div>
			</PageLayout.DragChildren>
			<div className={classes.honeycombOuter}>
				<div className={clsx(classes.honeycombContainer, 'mt-8')}>
					<DropItem
						name="Jamie's MacBook Pro"
						receivingNodeOsType="macOs"
						connectionType="lan"
						icon={Laptop}
					/>
					<DropItem
						name="Jamie's iPhone"
						receivingNodeOsType="iOS"
						connectionType="lan"
						icon={DeviceMobile}
					/>
					<DropItem
						name="Titan NAS"
						receivingNodeOsType="linux"
						connectionType="p2p"
						icon={HardDrives}
					/>
					<DropItem
						name="Jamie's iPad"
						receivingNodeOsType="iOS"
						connectionType="lan"
						icon={DeviceMobile}
					/>
					<DropItem name="Jamie's Google Drive" brandIcon="google-drive" connectionType="cloud" />
					<DropItem name="Jamie's iCloud" brandIcon="icloud" connectionType="cloud" />
					<DropItem name="Mega" brandIcon="mega" connectionType="cloud" />
					<DropItem
						name="maxichrome"
						image="https://github.com/maxichrome.png"
						connectionType="p2p"
					/>
					<DropItem
						name="Brendan Alan"
						image="https://github.com/brendonovich.png"
						connectionType="p2p"
					/>
					<DropItem
						name="Oscar Beaumont"
						image="https://github.com/oscartbeaumont.png"
						connectionType="p2p"
					/>
					<DropItem name="Polar" image="https://github.com/polargh.png" connectionType="p2p" />
					<DropItem
						name="Andrew Haskell"
						image="https://github.com/andrewtechx.png"
						connectionType="p2p"
					/>
				</div>
			</div>
		</>
	);
};
