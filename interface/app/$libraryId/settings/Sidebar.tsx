import {
	Books,
	FlyingSaucer,
	GearSix,
	HardDrive,
	Key,
	KeyReturn,
	PaintBrush,
	PuzzlePiece,
	Receipt,
	ShareNetwork,
	ShieldCheck,
	TagSimple
} from 'phosphor-react';
import { useFeatureFlag } from '@sd/client';
import { tw } from '@sd/ui';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import Icon from '../Layout/Sidebar/Icon';
import SidebarLink from '../Layout/Sidebar/Link';
import { NavigationButtons } from '../TopBar/NavigationButtons';

const Heading = tw.div`mb-1 ml-1 text-xs font-semibold text-gray-400`;
const Section = tw.div`space-y-0.5`;

export default () => {
	const os = useOperatingSystem();
	const isPairingEnabled = useFeatureFlag('p2pPairing');

	return (
		<div className="custom-scroll no-scrollbar h-full w-60 max-w-[180px] shrink-0 border-r border-app-line/50 pb-5">
			{os !== 'browser' ? (
				<div data-tauri-drag-region className="mb-3 h-3 w-full p-3 pl-[14px] pt-[10px]">
					<NavigationButtons />
				</div>
			) : (
				<div className="h-3" />
			)}

			<div className="space-y-6 px-4 py-3">
				<Section>
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
					<SidebarLink to="client/keybindings" disabled>
						<Icon component={KeyReturn} />
						Keybinds
					</SidebarLink>
					<SidebarLink to="client/extensions" disabled>
						<Icon component={PuzzlePiece} />
						Extensions
					</SidebarLink>
				</Section>
				<Section>
					<Heading>Library</Heading>
					<SidebarLink to="library/general">
						<Icon component={GearSix} />
						General
					</SidebarLink>
					<SidebarLink to="library/nodes" disabled={!isPairingEnabled}>
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
					<SidebarLink to="library/keys" disabled>
						<Icon component={Key} />
						Keys
					</SidebarLink>
				</Section>
				<Section>
					<Heading>Resources</Heading>
					<SidebarLink to="resources/about">
						<Icon component={FlyingSaucer} />
						About
					</SidebarLink>
					<SidebarLink to="resources/changelog">
						<Icon component={Receipt} />
						Changelog
					</SidebarLink>
					{/* <SidebarLink to="resources/dependencies">
						<Icon component={Graph} />
						Dependencies
					</SidebarLink>
					<SidebarLink to="resources/support">
						<Icon component={Heart} />
						Support
					</SidebarLink> */}
				</Section>
			</div>
		</div>
	);
};
