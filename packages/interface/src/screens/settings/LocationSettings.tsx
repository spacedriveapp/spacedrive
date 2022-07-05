import { useBridgeQuery } from '@sd/client';
import React from 'react';

import LocationListItem from '../../components/location/LocationListItem';
import { InputContainer } from '../../components/primitive/InputContainer';
import { SettingsContainer } from '../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../components/settings/SettingsHeader';

// const exampleLocations = [
// 	{ option: 'Macintosh HD', key: 'macintosh_hd' },
// 	{ option: 'LaCie External', key: 'lacie_external' },
// 	{ option: 'Seagate 8TB', key: 'seagate_8tb' }
// ];

export default function LocationSettings() {
	const { data: locations } = useBridgeQuery('SysGetLocations');

	console.log({ locations });

	return (
		<SettingsContainer>
			{/*<Button size="sm">Add Location</Button>*/}
			<SettingsHeader title="Locations" description="Manage your storage locations." />

			<div className="grid space-y-2">
				{(locations || []).map((location) => (
					<LocationListItem key={location.id} location={location} />
				))}
			</div>
		</SettingsContainer>
	);
}
