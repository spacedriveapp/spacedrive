import { CogIcon, KeyIcon, TagIcon } from '@heroicons/react/outline';
import { HardDrive } from 'phosphor-react';
import React from 'react';

import { SidebarLink } from '../../components/file/Sidebar';
import {
	SettingsHeading,
	SettingsIcon,
	SettingsScreenContainer
} from '../../components/settings/SettingsScreenContainer';

export const CurrentLibrarySettings: React.FC = () => {
	return (
		<SettingsScreenContainer>
			<SettingsHeading className="!mt-0">Library Settings</SettingsHeading>
			<SidebarLink to="/library-settings/general">
				<SettingsIcon component={CogIcon} />
				General
			</SidebarLink>
			<SidebarLink to="/library-settings/locations">
				<SettingsIcon component={HardDrive} />
				Locations
			</SidebarLink>
			<SidebarLink to="/library-settings/tags">
				<SettingsIcon component={TagIcon} />
				Tags
			</SidebarLink>
			<SidebarLink to="/library-settings/keys">
				<SettingsIcon component={KeyIcon} />
				Keys
			</SidebarLink>
		</SettingsScreenContainer>
	);
};
