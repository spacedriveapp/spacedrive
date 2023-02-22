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
import { useOperatingSystem } from '../../hooks/useOperatingSystem';
import { SidebarLink } from '../layout/Sidebar';
import { SettingsHeading, SettingsIcon } from './SettingsHeader';

export const SettingsSidebar = () => {
	const os = useOperatingSystem();
	return (
		<div className="border-app-line/50 custom-scroll no-scrollbar h-full w-60 max-w-[180px] shrink-0 border-r pb-5">
			{os !== 'browser' ? (
				<div data-tauri-drag-region className="h-5 w-full" />
			) : (
				<div className="h-3" />
			)}
			<div className="px-4 pb-2.5">
				<SettingsHeading className="!mt-2">Client</SettingsHeading>
				<SidebarLink to="general">
					<SettingsIcon component={GearSix} />
					General
				</SidebarLink>
				<SidebarLink to="libraries">
					<SettingsIcon component={Books} />
					Libraries
				</SidebarLink>
				<SidebarLink to="privacy">
					<SettingsIcon component={ShieldCheck} />
					Privacy
				</SidebarLink>
				<SidebarLink to="appearance">
					<SettingsIcon component={PaintBrush} />
					Appearance
				</SidebarLink>
				<SidebarLink to="keybindings">
					<SettingsIcon component={KeyReturn} />
					Keybinds
				</SidebarLink>
				<SidebarLink to="extensions">
					<SettingsIcon component={PuzzlePiece} />
					Extensions
				</SidebarLink>

				<SettingsHeading>Library</SettingsHeading>
				<SidebarLink to="library">
					<SettingsIcon component={GearSix} />
					General
				</SidebarLink>
				<SidebarLink to="nodes">
					<SettingsIcon component={ShareNetwork} />
					Nodes
				</SidebarLink>
				<SidebarLink to="locations">
					<SettingsIcon component={HardDrive} />
					Locations
				</SidebarLink>
				<SidebarLink to="tags">
					<SettingsIcon component={TagSimple} />
					Tags
				</SidebarLink>
				<SidebarLink to="keys">
					<SettingsIcon component={Key} />
					Keys
				</SidebarLink>
				<SettingsHeading>Resources</SettingsHeading>
				<SidebarLink to="about">
					<SettingsIcon component={FlyingSaucer} />
					About
				</SidebarLink>
				<SidebarLink to="changelog">
					<SettingsIcon component={Receipt} />
					Changelog
				</SidebarLink>
				<SidebarLink to="dependencies">
					<SettingsIcon component={Graph} />
					Dependencies
				</SidebarLink>
				<SidebarLink to="support">
					<SettingsIcon component={Heart} />
					Support
				</SidebarLink>
			</div>
		</div>
	);
};
