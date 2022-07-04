import { PlusIcon } from '@heroicons/react/solid';
import { useBridgeQuery, useLibraryCommand, useLibraryQuery } from '@sd/client';
import { Button } from '@sd/ui';
import React, { useContext } from 'react';

import { AppPropsContext } from '../../../AppPropsContext';
import LocationListItem from '../../../components/location/LocationListItem';
import { InputContainer } from '../../../components/primitive/InputContainer';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

// const exampleLocations = [
// 	{ option: 'Macintosh HD', key: 'macintosh_hd' },
// 	{ option: 'LaCie External', key: 'lacie_external' },
// 	{ option: 'Seagate 8TB', key: 'seagate_8tb' }
// ];

export default function LocationSettings() {
	const { data: locations } = useLibraryQuery('SysGetLocations');

	const appProps = useContext(AppPropsContext);

	const { mutate: createLocation } = useLibraryCommand('LocCreate');

	return (
		<SettingsContainer>
			{/*<Button size="sm">Add Location</Button>*/}
			<SettingsHeader
				title="Locations"
				description="Manage your storage locations."
				rightArea={
					<div className="flex-row space-x-2">
						<Button
							variant="primary"
							size="sm"
							onClick={() => {
								appProps?.openDialog({ directory: true }).then((result) => {
									if (result) createLocation({ path: result as string });
								});
							}}
						>
							Add Location
						</Button>
					</div>
				}
			/>

			<div className="grid space-y-2">
				{locations?.map((location) => (
					<LocationListItem key={location.id} location={location} />
				))}
			</div>
		</SettingsContainer>
	);
}
