import LocationListItem from '../../../components/location/LocationListItem';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { AppPropsContext } from '@sd/client';
import { LocationCreateArgs } from '@sd/core';
import { Button } from '@sd/ui';
import React, { useContext } from 'react';

export default function LocationSettings() {
	const { data: locations } = useLibraryQuery(['locations.list']);
	const { mutate: createLocation } = useLibraryMutation('locations.create');

	const appProps = useContext(AppPropsContext);

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
									// TODO: Pass indexer rules ids to create location
									if (result)
										createLocation({
											path: result as string,
											indexer_rules_ids: []
										} as LocationCreateArgs);
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
