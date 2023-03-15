import { GoogleDrive, Mega, iCloud } from '@sd/assets/images';
import clsx from 'clsx';
import { DeviceMobile, HardDrives, Icon, Laptop, User } from 'phosphor-react';
import { useRef, useState } from 'react';
import { tw } from '@sd/ui';
import { PeerMetadata, useBridgeMutation, useBridgeSubscription } from '~/../packages/client/src';
import { SubtleButton, SubtleButtonContainer } from '~/components/SubtleButton';
import { OperatingSystem } from '~/util/Platform';
import { SearchBar } from './Explorer/TopBar';
import * as PageLayout from './PageLayout';
import classes from './spacedrop.module.scss';

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
				{/* <SubtleButtonContainer className="absolute left-[12px] top-[55px]">
					<SubtleButton icon={Star} />
				</SubtleButtonContainer> */}
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

// TODO: This will be removed and properly hooked up to the UI in the future
function TemporarySpacedropDemo() {
	const [[discoveredPeers], setDiscoveredPeer] = useState([new Map<string, PeerMetadata>()]);
	const doSpacedrop = useBridgeMutation('p2p.spacedrop');

	useBridgeSubscription(['p2p.events'], {
		onData(data) {
			if (data.type === 'DiscoveredPeer') {
				setDiscoveredPeer([discoveredPeers.set(data.peer_id, data.metadata)]);
			}
		}
	});

	console.log(discoveredPeers);

	// TODO: Input select
	return (
		<form
			onSubmit={(e) => {
				e.preventDefault();
				doSpacedrop.mutate({
					peer_id: e.currentTarget.targetPeer.value,
					file_path: e.currentTarget.filePath.value
				});
			}}
		>
			<h1 className="mt-4 text-4xl">Spacedrop Demo</h1>
			<p>
				Note: Right now the file must be less than 255 bytes long and only contain UTF-8 chars.
				Create a txt file in Vscode to test (note macOS TextEdit cause that is rtf by default)
			</p>
			<select id="targetPeer" name="targetPeer" className="my-2 w-full text-black">
				{[...discoveredPeers.entries()].map(([peerId, metdata]) => (
					<option key={peerId} value={peerId}>
						{metdata.name}
					</option>
				))}
			</select>
			<input
				name="filePath"
				placeholder="file path"
				value="/Users/oscar/Desktop/sd/demo.txt"
				onChange={() => {}}
				className="my-2 w-full p-2 text-black"
			/>
			<input
				type="submit"
				value="Full send it!"
				className="mt-4 h-10 w-32 rounded-full bg-red-500"
			/>
		</form>
	);
}

export const Component = () => {
	const searchRef = useRef<HTMLInputElement>(null);

	return (
		<>
			<TemporarySpacedropDemo />
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
						receivingNodeOsType="macOS"
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
