import { useCache, useLibraryQuery, useNodes, usePathsExplorerQuery } from '@sd/client';
import { useEffect } from 'react';
import Explorer from '~/components/explorer/Explorer';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';
import { getExplorerStore } from '~/stores/explorerStore';

export default function LocationScreen({ navigation, route }: BrowseStackScreenProps<'Location'>) {
	const { id, path } = route.params;

	const location = useLibraryQuery(['locations.get', route.params.id]);
	useNodes(location.data?.nodes);
	const locationData = useCache(location.data?.item);

	const paths = usePathsExplorerQuery({
		arg: {
			filters: [
				// ...search.allFilters,
				{ filePath: { locations: { in: [id] } } },
				{
					filePath: {
						path: {
							location_id: id,
							path: path ?? '',
							include_descendants: false
							// include_descendants:
							// 	search.search !== '' ||
							// 	search.dynamicFilters.length > 0 ||
							// 	(layoutMode === 'media' && mediaViewWithDescendants)
						}
					}
				}
				// !showHiddenFiles && { filePath: { hidden: false } }
			].filter(Boolean) as any,
			take: 30
		},
		order: null,
		onSuccess: () => getExplorerStore().resetNewThumbnails()
	});

	useEffect(() => {
		// Set screen title to location.
		if (path && path !== '') {
			// Nested location.
			navigation.setOptions({
				title: path
					.split('/')
					.filter((x) => x !== '')
					.pop()
			});
		} else {
			navigation.setOptions({
				title: locationData?.name ?? 'Location'
			});
		}
	}, [locationData?.name, navigation, path]);

	useEffect(() => {
		getExplorerStore().locationId = id;
		getExplorerStore().path = path ?? '';
	}, [id, path]);

	return <Explorer {...paths} />;
}
