import { GoogleDrive, Mega, iCloud } from '@sd/assets/images';
import clsx from 'clsx';
import { DeviceMobile, HardDrives, Icon, Laptop, User } from 'phosphor-react';
<<<<<<< HEAD
import { useRef, useState } from 'react';
import { Button, Card, Label, Loader, Select, SelectOption, forms, tw } from '@sd/ui';
import { PeerMetadata, useBridgeMutation, useBridgeSubscription } from '~/../packages/client/src';
import { SubtleButton, SubtleButtonContainer } from '~/components/SubtleButton';
import { OperatingSystem } from '~/util/Platform';
import DragRegion from '../../components/DragRegion';
import { SearchBar } from './Explorer/TopBar';
import * as PageLayout from './PageLayout';
=======
import { useState } from 'react';
import { Button, ScreenHeading, Select, SelectOption, forms, tw } from '@sd/ui';
import { PeerMetadata, useBridgeMutation, useBridgeSubscription } from '~/../packages/client/src';
import { SubtleButton, SubtleButtonContainer } from '~/components/SubtleButton';
import { OperatingSystem } from '~/util/Platform';
>>>>>>> origin/main
import classes from './spacedrop.module.scss';

const { Form, Input, useZodForm, z } = forms;

const spacedropSchema = z.object({
	target_peer: z.string(),
	file_path: z.string()
});

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
<<<<<<< HEAD
		<div className={clsx(classes.honeycombItem, 'overflow-hidden ', ' hover:bg-app-box/50')}>
=======
		<div
			className={clsx(
				classes.honeycombItem,
				'overflow-hidden bg-app-box/20 hover:bg-app-box/50'
			)}
		>
>>>>>>> origin/main
			<div className="group relative flex h-full w-full flex-col items-center justify-center ">
				{/* <SubtleButtonContainer className="absolute left-[12px] top-[55px]">
					<SubtleButton icon={Star} />
				</SubtleButtonContainer> */}
				<div className="h-14 w-14 rounded-full bg-app-button">{icon}</div>
				{/* <SubtleButtonContainer className="absolute right-[12px] top-[55px] rotate-90">
					<SubtleButton />
				</SubtleButtonContainer> */}
				{props.name && (
					<span className="mt-1 truncate text-center text-xs font-medium">{props.name}</span>
				)}
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

// // TODO: This will be removed and properly hooked up to the UI in the future
// function TemporarySpacedropDemo() {

// 	// TODO: Input select
// 	return (
// 		<Form onSubmit={onSubmit} form={form}>
// 			<h1 className="mt-4 text-2xl font-bold">Spacedrop Demo</h1>
// 			<p className="text-xs text-ink-dull">
// 				Note: Right now the file must be less than 255 bytes long and only contain UTF-8 chars.
// 				Create a txt file in Vscode to test (note macOS TextEdit cause that is rtf by default)
// 			</p>
// 			<div className="mt-2 flex flex-row items-center space-x-4">
// 				<Input
// 					size="sm"
// 					placeholder="/Users/oscar/Desktop/sd/demo.txt"
// 					value="/Users/jamie/Desktop/Jeff.txt"
// 					className="w-full"
// 					{...form.register('file_path')}
// 				/>

// 				<Button className="block flex-shrink-0" variant="gray">
// 					Select File
// 				</Button>

// 				<Select onChange={(e) => form.setValue('target_peer', e)} value={form.watch('target_peer')}>
// 					{[...discoveredPeers.entries()].map(([peerId, metadata], index) => (
// 						<SelectOption default={index === 0} key={peerId} value={peerId}>
// 							{metadata.name}
// 						</SelectOption>
// 					))}
// 				</Select>

// 				<Button
// 					disabled={!form.getValues().target_peer}
// 					className="block flex-shrink-0"
// 					variant="accent"
// 					type="submit"
// 				>
// 					Send
// 				</Button>
// 			</div>
// 		</Form>
// 	);
// }

export const Component = () => {
	const searchRef = useRef<HTMLInputElement>(null);
	const [[discoveredPeers], setDiscoveredPeer] = useState([new Map<string, PeerMetadata>()]);
	const doSpacedrop = useBridgeMutation('p2p.spacedrop');

	const form = useZodForm({
		schema: spacedropSchema
	});

	useBridgeSubscription(['p2p.events'], {
		onData(data) {
			if (data.type === 'DiscoveredPeer') {
				setDiscoveredPeer([discoveredPeers.set(data.peer_id, data.metadata)]);
				// if (!form.getValues().target_peer) form.setValue('target_peer', data.peer_id);
			}
		}
	});

	const onSubmit = form.handleSubmit((data) => {
		doSpacedrop.mutate({
			peer_id: data.target_peer,
			file_path: data.file_path
		});
	});
<<<<<<< HEAD
	return (
		<>
			{discoveredPeers.size === 0 && (
				<div className="flex h-full w-full flex-col items-center justify-center">
					<Loader className="h-8 w-8" />
					<span className="mt-2 text-xs text-ink-dull">Searching for peers...</span>
				</div>
			)}
			<Card className="absolute bottom-5 text-xs text-ink-dull">
				Note: Currently the file must be less than 255 bytes long and only contain UTF-8 chars.
				Create a txt file in Vscode to test (note macOS TextEdit cause that is rtf by default) Also,
				only local LAN peers are currently supported.
			</Card>
			<Form form={form} onSubmit={onSubmit}>
				<div className={classes.honeycombOuter}>
					<div className={clsx(classes.honeycombContainer, 'mt-0')}>
						{[...discoveredPeers.entries()].map(([peerId, metadata], index) => (
							<DropItem
								key={peerId}
								name={metadata.name}
								receivingNodeOsType={metadata.operating_system}
								icon={Laptop}
							/>
						))}

						{/* <DropItem
=======

	// TODO: Input select
	return (
		<Form onSubmit={onSubmit} form={form}>
			<ScreenHeading>Spacedrop Demo</ScreenHeading>
			<p className="text-xs text-ink-dull">
				Note: Right now the file must be less than 255 bytes long and only contain UTF-8
				chars. Create a txt file in Vscode to test (note macOS TextEdit cause that is rtf by
				default)
			</p>
			<div className="mt-2 flex flex-row items-center space-x-4">
				<Input
					size="sm"
					placeholder="/Users/oscar/Desktop/sd/demo.txt"
					value="/Users/jamie/Desktop/Jeff.txt"
					className="w-full"
					{...form.register('file_path')}
				/>

				<Button className="block shrink-0" variant="gray">
					Select File
				</Button>

				<Select
					onChange={(e) => form.setValue('target_peer', e)}
					value={form.watch('target_peer')}
				>
					{[...discoveredPeers.entries()].map(([peerId, metadata], index) => (
						<SelectOption default={index === 0} key={peerId} value={peerId}>
							{metadata.name}
						</SelectOption>
					))}
				</Select>

				<Button
					disabled={!form.getValues().target_peer}
					className="block shrink-0"
					variant="accent"
					type="submit"
				>
					Send
				</Button>
			</div>
		</Form>
	);
}

export const Component = () => {
	return (
		<>
			<TemporarySpacedropDemo />
			<div className={classes.honeycombOuter}>
				<div className={clsx(classes.honeycombContainer, 'mt-8')}>
					<DropItem
						name="Jamie's MacBook Pro"
						receivingNodeOsType="macOS"
						connectionType="lan"
						icon={Laptop}
					/>
					<DropItem
>>>>>>> origin/main
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
					<DropItem
						name="Jamie's Google Drive"
						brandIcon="google-drive"
						connectionType="cloud"
					/>
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
					<DropItem
						name="Polar"
						image="https://github.com/polargh.png"
						connectionType="p2p"
					/>
					<DropItem
						name="Andrew Haskell"
						image="https://github.com/andrewtechx.png"
						connectionType="p2p"
					/> */}
					</div>
				</div>
			</Form>
		</>
	);
};
