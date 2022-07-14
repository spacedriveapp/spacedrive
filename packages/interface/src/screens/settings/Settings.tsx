import {
	CogIcon,
	CollectionIcon,
	DatabaseIcon,
	GlobeAltIcon,
	HeartIcon,
	InformationCircleIcon,
	KeyIcon,
	LibraryIcon,
	LightBulbIcon,
	TagIcon,
	TerminalIcon
} from '@heroicons/react/outline';
import {
	BookOpen,
	Cloud,
	HardDrive,
	Hash,
	Info,
	KeyReturn,
	PaintBrush,
	PuzzlePiece,
	ShareNetwork,
	UsersFour
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
			<SidebarLink to="/settings/appearance">
				<SettingsIcon component={PaintBrush} />
				Appearance
			</SidebarLink>
			<SidebarLink to="/settings/keybinds">
				<SettingsIcon component={KeyReturn} />
				Keybinds
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
			<SettingsHeading>Advanced</SettingsHeading>
			<SidebarLink to="/settings/p2p">
				<SettingsIcon component={ShareNetwork} />
				Networking
			</SidebarLink>
			<SidebarLink to="/settings/experimental">
				<SettingsIcon component={TerminalIcon} />
				Developer
			</SidebarLink>

			<SettingsHeading>Resources</SettingsHeading>
			<SidebarLink to="/settings/about">
				<SettingsIcon component={BookOpen} />
				About
			</SidebarLink>
			<SidebarLink to="/settings/changelog">
				<SettingsIcon component={LightBulbIcon} />
				Changelog
			</SidebarLink>
			<SidebarLink to="/settings/support">
				<SettingsIcon component={HeartIcon} />
				Support
			</SidebarLink>
		</SettingsScreenContainer>
	);
};
