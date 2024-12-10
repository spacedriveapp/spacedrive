import { ObjectOrder, objectOrderingKeysSchema, useLibraryQuery } from '@sd/client';

import { ItemsCard } from './ItemsCard';

const RecentFiles = () => {
	const recentItemsQuery = useLibraryQuery([
		'search.objects',
		{
			take: 20,
			orderAndPagination: {
				orderOnly: { field: 'dateAccessed', value: 'Desc' }
			},
			filters: [{ object: { dateAccessed: { from: new Date(0).toISOString() } } }]
		}
	]);

	return (
		<ItemsCard
			title="Recent Items"
			query={recentItemsQuery}
			buttonText="See all recent items"
			buttonLink="/explorer"
		/>
	);
};

export default RecentFiles;
