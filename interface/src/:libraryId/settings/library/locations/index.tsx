import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { LocationCreateArgs } from '@sd/client';
import { Button, SearchInput, dialogManager } from '@sd/ui';
import { RouteObject } from 'react-router';
import AddLocationDialog from '~/components/dialog/AddLocationDialog';
import LocationListItem from '~/components/location/LocationListItem';
import { SettingsContainer } from '~/components/settings/SettingsContainer';
import { SettingsHeader } from '~/components/settings/SettingsHeader';
import { lazyEl } from '~/util';
import { usePlatform } from '~/util/Platform';

function LocationSettings() {
	const platform = usePlatform();
	const locations = useLibraryQuery(['locations.list']);
	const createLocation = useLibraryMutation('locations.create');

	return (
		<SettingsContainer>
			<SettingsHeader
				title="Locations"
				description="Manage your storage locations."
				rightArea={
					<div className="flex flex-row items-center space-x-5">
						<SearchInput placeholder="Search locations" />

						<Button
							variant="accent"
							size="md"
							onClick={() => {
								if (platform.platform === 'web') {
									dialogManager.create((dp) => <AddLocationDialog {...dp} />);
								} else {
									if (!platform.openDirectoryPickerDialog) {
										alert('Opening a dialogue is not supported on this platform!');
										return;
									}
									platform.openDirectoryPickerDialog().then((result) => {
										// TODO: Pass indexer rules ids to create location
										if (result)
											createLocation.mutate({
												path: result as string,
												indexer_rules_ids: []
											} as LocationCreateArgs);
									});
								}
							}}
						>
							Add Location
						</Button>
					</div>
				}
			/>
			<div className="grid space-y-2">
				{locations.data?.map((location) => (
					<LocationListItem key={location.id} location={location} />
				))}
			</div>
		</SettingsContainer>
	);
}

export default [
	{ index: true, element: <LocationSettings/> },
	{ path: ':id', element: lazyEl(() => import('./:id')) }
] satisfies RouteObject[];
