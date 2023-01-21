import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { LocationCreateArgs } from '@sd/client';
import { Button, Input } from '@sd/ui';
import { MagnifyingGlass } from 'phosphor-react';
import { useState } from 'react';

import AddLocationDialog from '~/components/dialog/AddLocationDialog';
import LocationListItem from '~/components/location/LocationListItem';
import { SettingsContainer } from '~/components/settings/SettingsContainer';
import { SettingsHeader } from '~/components/settings/SettingsHeader';
import { usePlatform } from '~/util/Platform';

export default function LocationSettings() {
	const platform = usePlatform();
	const { data: locations } = useLibraryQuery(['locations.list']);
	const { mutate: createLocation } = useLibraryMutation('locations.create');
	const [textLocationDialogOpen, setTextLocationDialogOpen] = useState(false);

	return (
		<SettingsContainer>
			{/*<Button size="sm">Add Location</Button>*/}
			<SettingsHeader
				title="Locations"
				description="Manage your storage locations."
				rightArea={
					<div className="flex flex-row items-center space-x-5">
						<div className="relative hidden lg:block">
							<MagnifyingGlass className="absolute w-[18px] h-auto top-[8px] left-[11px] text-gray-350" />
							<Input className="!p-0.5 !pl-9" placeholder="Search locations" />
						</div>
						<AddLocationDialog open={textLocationDialogOpen} setOpen={setTextLocationDialogOpen} />
						<Button
							variant="accent"
							size="sm"
							onClick={() => {
								if (platform.platform === 'web') {
									setTextLocationDialogOpen(true);
								} else {
									if (!platform.openDirectoryPickerDialog) {
										alert('Opening a dialogue is not supported on this platform!');
										return;
									}
									platform.openDirectoryPickerDialog().then((result) => {
										// TODO: Pass indexer rules ids to create location
										if (result)
											createLocation({
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
				{locations?.map((location) => (
					<LocationListItem key={location.id} location={location} />
				))}
			</div>
		</SettingsContainer>
	);
}
