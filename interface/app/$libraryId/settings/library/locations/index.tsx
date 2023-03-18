import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { SearchInput } from '@sd/ui';
import { usePlatform } from '~/util/Platform';
import { Heading } from '../../Layout';
import { AddLocationButton } from './AddLocationButton';
import ListItem from './ListItem';

export const Component = () => {
	const platform = usePlatform();
	const locations = useLibraryQuery(['locations.list']);
	const createLocation = useLibraryMutation('locations.create');

	return (
		<>
			<Heading
				title="Locations"
				description="Manage your storage locations."
				rightArea={
					<div className="flex flex-row items-center space-x-5">
						<SearchInput placeholder="Search locations" />
						<AddLocationButton variant="accent" size="md"></AddLocationButton>
					</div>
				}
			/>
			<div className="grid space-y-2">
				{locations.data?.map((location) => (
					<ListItem key={location.id} location={location} />
				))}
			</div>
		</>
	);
};
