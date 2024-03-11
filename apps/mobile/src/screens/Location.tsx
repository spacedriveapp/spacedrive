import { useEffect, useMemo } from 'react';
import { useCache, useLibraryQuery, useNodes } from '@sd/client';
import Explorer from '~/components/explorer/Explorer';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';
import { getExplorerStore } from '~/stores/explorerStore';

export default function LocationScreen({ navigation, route }: BrowseStackScreenProps<'Location'>) {
	const { id, path } = route.params;

	const location = useLibraryQuery(['locations.get', route.params.id]);
	useNodes(location.data?.nodes);
	const locationData = useCache(location.data?.item);

	const paths = useLibraryQuery([
		'search.paths',
		{
			filters: [
				{
					filePath: {
						path: { location_id: id, path: path ?? '', include_descendants: true }
					}
				}
			],
			take: 100
		}
	]);

	const pathsItemsReferences = useMemo(() => paths.data?.items ?? [], [paths.data]);
	useNodes(paths.data?.nodes);
	const pathsItems = useCache(pathsItemsReferences);

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

	return <Explorer items={pathsItems} />;
}
