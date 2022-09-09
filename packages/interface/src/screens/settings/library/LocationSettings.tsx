import { useLibraryMutation, useLibraryQuery, usePlatform } from '@sd/client';
import { LocationCreateArgs } from '@sd/core';
import { Button } from '@sd/ui';

import LocationListItem from '../../../components/location/LocationListItem';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function LocationSettings() {
	const platform = usePlatform();
	const { data: locations } = useLibraryQuery(['locations.list']);
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
								if (!platform.openFilePickerDialog) {
									// TODO: Support opening locations on web
									alert('Opening a dialogue is not supported on this platform!');
									return;
								}

								platform.openFilePickerDialog().then((result) => {
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
