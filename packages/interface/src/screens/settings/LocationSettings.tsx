import { Button } from '@sd/ui';
import React from 'react';

import { InputContainer } from '../../components/primitive/InputContainer';
import { SettingsContainer } from '../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../components/settings/SettingsHeader';

const exampleLocations = [
	{ option: 'Macintosh HD', key: 'macintosh_hd' },
	{ option: 'LaCie External', key: 'lacie_external' },
	{ option: 'Seagate 8TB', key: 'seagate_8tb' }
];

export default function LocationSettings() {
	// const locations = useBridgeQuery("SysGetLocation")

	return (
		<SettingsContainer>
			{/*<Button size="sm">Add Location</Button>*/}
			<SettingsHeader title="Locations" description="Manage your settings related to locations." />
			<InputContainer
				title="Something about a vault"
				description="Local cache storage for media previews and thumbnails."
			>
				<div className="flex flex-row space-x-2"></div>
			</InputContainer>
		</SettingsContainer>
	);
}
