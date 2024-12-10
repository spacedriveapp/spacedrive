import { useLibraryQuery } from '@sd/client';

import { ItemsCard } from './ItemsCard';

const FavoriteItems = () => {
	const favoriteItemsQuery = useLibraryQuery([
		'search.objects',
		{
			take: 6,
			orderAndPagination: {
				orderOnly: { field: 'dateAccessed', value: 'Desc' }
			},
			filters: [{ object: { favorite: true } }]
		}
	]);

	return (
		<ItemsCard
			title="Favorite Items"
			query={favoriteItemsQuery}
			buttonText="See all favorites"
			buttonLink="/explorer?filter=favorites"
		/>
	);
};

export default FavoriteItems;
