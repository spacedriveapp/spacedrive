import {
	Books,
	FlyingSaucer,
	GearSix,
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
		<div className="h-full border-r max-w-[180px] flex-shrink-0 border-app-line/50 w-60 custom-scroll no-scrollbar pb-5">
			{os !== 'browser' ? (
				<div data-tauri-drag-region className="w-full h-5" />
			) : (
				<div className="h-3" />
			)}
			<div className="px-4 pb-2.5">
				<SettingsHeading className="!mt-2">Client</SettingsHeading>
				<SidebarLink to="/settings/general">
					<SettingsIcon component={GearSix} />
					General
				</SidebarLink>
				<SidebarLink to="/settings/libraries">
					<SettingsIcon component={Books} />
					Libraries
				</SidebarLink>
				<SidebarLink to="/settings/privacy">
					<SettingsIcon component={ShieldCheck} />
					Privacy
				</SidebarLink>
				<SidebarLink to="/settings/appearance">
					<SettingsIcon component={PaintBrush} />
					Appearance
				</SidebarLink>
				<SidebarLink to="/settings/keybindings">
					<SettingsIcon component={KeyReturn} />
					Keybinds
				</SidebarLink>
				<SidebarLink to="/settings/extensions">
					<SettingsIcon component={PuzzlePiece} />
					Extensions
				</SidebarLink>

				<SettingsHeading>Library</SettingsHeading>
				<SidebarLink to="/settings/library">
					<SettingsIcon component={GearSix} />
					General
				</SidebarLink>
				<SidebarLink to="/settings/nodes">
					<SettingsIcon component={ShareNetwork} />
					Nodes
				</SidebarLink>
				<SidebarLink to="/settings/locations">
					<SettingsIcon component={HardDrive} />
					Locations
				</SidebarLink>
				<SidebarLink to="/settings/tags">
					<SettingsIcon component={TagSimple} />
					Tags
				</SidebarLink>
				<SidebarLink to="/settings/keys">
					<SettingsIcon component={Key} />
					Keys
				</SidebarLink>
				<SettingsHeading>Resources</SettingsHeading>
				<SidebarLink to="/settings/about">
					<SettingsIcon component={FlyingSaucer} />
					About
				</SidebarLink>
				<SidebarLink to="/settings/changelog">
					<SettingsIcon component={Receipt} />
					Changelog
				</SidebarLink>
				<SidebarLink to="/settings/support">
					<SettingsIcon component={Heart} />
					Support
				</SidebarLink>
			</div>
		</div>
	);
};
