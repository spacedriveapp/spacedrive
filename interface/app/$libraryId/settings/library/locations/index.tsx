import { useLibraryQuery } from '@sd/client';
import { SearchInput } from '@sd/ui';
import { Heading } from '../../Layout';
import { AddLocationButton } from './AddLocationButton';
import ListItem from './ListItem';

export const Component = () => {
	const locations = useLibraryQuery(['locations.list']);

	return (
		<>
			<Heading
				title="Locations"
				description="Manage your storage locations."
				rightArea={
					<div className="flex flex-row items-center space-x-5">
						<SearchInput placeholder="Search locations" className="h-[33px]" />
						<AddLocationButton variant="accent" size="md" />
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
