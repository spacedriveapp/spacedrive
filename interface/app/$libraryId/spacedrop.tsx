import { Icon, User } from '@phosphor-icons/react';
import { GoogleDrive, iCloud, Mega } from '@sd/assets/images';
import clsx from 'clsx';
import { tw } from '@sd/ui';
import { SubtleButton, SubtleButtonContainer } from '~/components';
import { OperatingSystem } from '~/util/Platform';

import classes from './spacedrop.module.scss';

// const { Form, Input, useZodForm, z } = forms;

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
			className={clsx(
				classes.honeycombItem,
				'overflow-hidden bg-app-box/20 hover:bg-app-box/50'
			)}
		>
			<div className="group relative flex h-full w-full flex-col items-center justify-center ">
				{/* <SubtleButtonContainer className="absolute left-[12px] top-[55px]">
					<SubtleButton icon={Star} />
				</SubtleButtonContainer> */}
				<div className="h-14 w-14 rounded-full bg-app-button">{icon}</div>
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

export const Component = () => {
	return (
		<>
			<div className={classes.honeycombOuter}>
				<div className={clsx(classes.honeycombContainer, 'mt-8')}></div>
			</div>
		</>
	);
};
