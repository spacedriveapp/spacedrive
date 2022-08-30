import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { AppPropsContext } from '@sd/client';
import { Button } from '@sd/ui';
import React, { useContext } from 'react';

import LocationListItem from '../../../components/location/LocationListItem';
import { InputContainer } from '../../../components/primitive/InputContainer';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function LocationSettings() {
	const { data: locations } = useLibraryQuery(['locations.get']);

	const appProps = useContext(AppPropsContext);

	const { mutate: createLocation } = useLibraryMutation('locations.create');

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
									if (result) createLocation(result as string);
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
