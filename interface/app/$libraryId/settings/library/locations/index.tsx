import { useMemo, useState } from 'react';
import { useDebounce } from 'use-debounce';
import { useLibraryQuery } from '@sd/client';
import { SearchInput } from '@sd/ui';

import { Heading } from '../../Layout';
import { AddLocationButton } from './AddLocationButton';
import ListItem from './ListItem';

export const Component = () => {
	const locations = useLibraryQuery(['locations.list']);

	const [search, setSearch] = useState('');
	const [debouncedSearch] = useDebounce(search, 200);

	const filteredLocations = useMemo(
		() =>
			locations.data?.filter(
				(location) => location.name?.toLowerCase().includes(debouncedSearch.toLowerCase())
			),
		[debouncedSearch, locations.data]
	);

	return (
		<>
			<Heading
				title="Locations"
				description="Manage your storage locations."
				rightArea={
					<div className="flex flex-row items-center space-x-5">
						<SearchInput
							placeholder="Search locations"
							className="h-[33px]"
							onChange={(e) => setSearch(e.target.value)}
						/>
						<AddLocationButton variant="accent" size="md" />
					</div>
				}
			/>
			<div className="grid space-y-2">
				{filteredLocations?.map((location) => (
					<ListItem key={location.id} location={location} />
				))}
			</div>
		</>
	);
};
