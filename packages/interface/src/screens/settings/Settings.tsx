import {
	CogIcon,
	CollectionIcon,
	GlobeAltIcon,
	KeyIcon,
	TerminalIcon
} from '@heroicons/react/outline';
import { HardDrive, PaintBrush, ShareNetwork } from 'phosphor-react';
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
			<SidebarLink to="/settings/appearance">
				<SettingsIcon component={PaintBrush} />
				Appearance
			</SidebarLink>

			<SettingsHeading>Node</SettingsHeading>
			<SidebarLink to="/settings/nodes">
				<SettingsIcon component={GlobeAltIcon} />
				Nodes
			</SidebarLink>
			<SidebarLink to="/settings/p2p">
				<SettingsIcon component={ShareNetwork} />
				P2P
			</SidebarLink>
			<SidebarLink to="/settings/library">
				<SettingsIcon component={CollectionIcon} />
				Libraries
			</SidebarLink>
			<SidebarLink to="/settings/security">
				<SettingsIcon component={KeyIcon} />
				Security
			</SidebarLink>
			<SettingsHeading>Developer</SettingsHeading>
			<SidebarLink to="/settings/experimental">
				<SettingsIcon component={TerminalIcon} />
				Experimental
			</SidebarLink>
			{/* <SettingsHeading>Library</SettingsHeading>
					<SidebarLink to="/settings/library">
						<SettingsIcon component={CollectionIcon} />
						My Libraries
					</SidebarLink>
					<SidebarLink to="/settings/locations">
						<SettingsIcon component={HardDrive} />
						Locations
					</SidebarLink>

					<SidebarLink to="/settings/keys">
						<SettingsIcon component={KeyIcon} />
						Keys
					</SidebarLink>
					<SidebarLink to="/settings/tags">
						<SettingsIcon component={TagIcon} />
						Tags
					</SidebarLink> */}

			{/* <SettingsHeading>Cloud</SettingsHeading>
					<SidebarLink to="/settings/sync">
						<SettingsIcon component={CloudIcon} />
						Sync
					</SidebarLink>
					<SidebarLink to="/settings/contacts">
						<SettingsIcon component={UsersIcon} />
						Contacts
					</SidebarLink> */}
		</SettingsScreenContainer>
	);
};
