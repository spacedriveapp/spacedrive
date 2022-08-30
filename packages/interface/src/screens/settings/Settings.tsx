import {
	AnnotationIcon,
	CogIcon,
	CollectionIcon,
	HeartIcon,
	KeyIcon,
	ShieldCheckIcon,
	TagIcon,
} from '@heroicons/react/outline';
import {
	FlyingSaucer,
	HardDrive,
	KeyReturn,
	PaintBrush,
	PuzzlePiece,
	ShareNetwork,
} from 'phosphor-react';
import React from 'react';

import { SidebarLink } from '../../components/file/Sidebar';
import {
	SettingsHeading,
	SettingsIcon,
	SettingsScreenContainer
} from '../../components/settings/SettingsScreenContainer';

export const SettingsScreen: React.FC = () => {
	return (
		<SettingsScreenContainer>
			<SettingsHeading className="!mt-0">Client</SettingsHeading>
			<SidebarLink to="/settings/general">
				<SettingsIcon component={CogIcon} />
				General
			</SidebarLink>
			<SidebarLink to="/settings/libraries">
				<SettingsIcon component={CollectionIcon} />
				Libraries
			</SidebarLink>
			<SidebarLink to="/settings/privacy">
				<SettingsIcon component={ShieldCheckIcon} />
				Privacy
			</SidebarLink>
			<SidebarLink to="/settings/appearance">
				<SettingsIcon component={PaintBrush} />
				Appearance
			</SidebarLink>
			<SidebarLink to="/settings/keybindings">
				<SettingsIcon component={KeyReturn} />
				Keybindings
			</SidebarLink>
			<SidebarLink to="/settings/extensions">
				<SettingsIcon component={PuzzlePiece} />
				Extensions
			</SidebarLink>

			<SettingsHeading>Library</SettingsHeading>
			<SidebarLink to="/settings/library">
				<SettingsIcon component={CogIcon} />
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
				<SettingsIcon component={TagIcon} />
				Tags
			</SidebarLink>
			<SidebarLink to="/settings/keys">
				<SettingsIcon component={KeyIcon} />
				Keys
			</SidebarLink>
			{/* <SidebarLink to="/settings/backups">
				<SettingsIcon component={DatabaseIcon} />
				Backups
			</SidebarLink>
			<SidebarLink to="/settings/backups">
				<SettingsIcon component={ShareNetwork} />
				Sync
			</SidebarLink> */}
			{/* <SettingsHeading>Advanced</SettingsHeading>
			<SidebarLink to="/settings/p2p">
				<SettingsIcon component={ShareNetwork} />
				Networking
			</SidebarLink>
			<SidebarLink to="/settings/experimental">
				<SettingsIcon component={TerminalIcon} />
				Developer
			</SidebarLink> */}

			<SettingsHeading>Resources</SettingsHeading>
			<SidebarLink to="/settings/about">
				<SettingsIcon component={FlyingSaucer} />
				About
			</SidebarLink>
			<SidebarLink to="/settings/changelog">
				<SettingsIcon component={AnnotationIcon} />
				Changelog
			</SidebarLink>
			<SidebarLink to="/settings/support">
				<SettingsIcon component={HeartIcon} />
				Support
			</SidebarLink>
		</SettingsScreenContainer>
	);
};
