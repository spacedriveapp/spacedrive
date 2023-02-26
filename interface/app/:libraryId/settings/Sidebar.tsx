import {
	Books,
	FlyingSaucer,
	GearSix,
	Graph,
	HardDrive,
	Heart,
	Key,
	KeyReturn,
	PaintBrush,
	PuzzlePiece,
	Receipt,
	ShareNetwork,
	ShieldCheck,
	TagSimple
} from 'phosphor-react';
import { tw } from '@sd/ui';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import Icon from '../Layout/Sidebar/Icon';
import SidebarLink from '../Layout/Sidebar/Link';

const Heading = tw.div`mt-5 mb-1 ml-1 text-xs font-semibold text-gray-400`;

export default () => {
	const os = useOperatingSystem();

	return (
		<div className="border-app-line/50 custom-scroll no-scrollbar h-full w-60 max-w-[180px] shrink-0 border-r pb-5">
			{os !== 'browser' ? (
				<div data-tauri-drag-region className="h-5 w-full" />
			) : (
				<div className="h-3" />
			)}
			<div className="px-4 pb-2.5 pt-2">
				<Heading>Client</Heading>
				<SidebarLink to="client/general">
					<Icon component={GearSix} />
					General
				</SidebarLink>
				<SidebarLink to="node/libraries">
					<Icon component={Books} />
					Libraries
				</SidebarLink>
				<SidebarLink to="client/privacy">
					<Icon component={ShieldCheck} />
					Privacy
				</SidebarLink>
				<SidebarLink to="client/appearance">
					<Icon component={PaintBrush} />
					Appearance
				</SidebarLink>
				<SidebarLink to="client/keybindings">
					<Icon component={KeyReturn} />
					Keybinds
				</SidebarLink>
				<SidebarLink to="client/extensions">
					<Icon component={PuzzlePiece} />
					Extensions
				</SidebarLink>

				<Heading>Library</Heading>
				<SidebarLink to="library/general">
					<Icon component={GearSix} />
					General
				</SidebarLink>
				<SidebarLink to="library/nodes">
					<Icon component={ShareNetwork} />
					Nodes
				</SidebarLink>
				<SidebarLink to="library/locations">
					<Icon component={HardDrive} />
					Locations
				</SidebarLink>
				<SidebarLink to="library/tags">
					<Icon component={TagSimple} />
					Tags
				</SidebarLink>
				<SidebarLink to="library/keys">
					<Icon component={Key} />
					Keys
				</SidebarLink>

				<Heading>Resources</Heading>
				<SidebarLink to="resources/about">
					<Icon component={FlyingSaucer} />
					About
				</SidebarLink>
				<SidebarLink to="resources/changelog">
					<Icon component={Receipt} />
					Changelog
				</SidebarLink>
				<SidebarLink to="resources/dependencies">
					<Icon component={Graph} />
					Dependencies
				</SidebarLink>
				<SidebarLink to="resources/support">
					<Icon component={Heart} />
					Support
				</SidebarLink>
			</div>
		</div>
	);
};
