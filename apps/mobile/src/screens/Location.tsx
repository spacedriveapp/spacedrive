import React, { useEffect, useMemo } from 'react';
import { useCache, useLibraryQuery } from '@sd/client';
import Explorer from '~/components/explorer/Explorer';
import { SharedScreenProps } from '~/navigation/SharedScreens';
import { getExplorerStore } from '~/stores/explorerStore';

export default function LocationScreen({ navigation, route }: SharedScreenProps<'Location'>) {
	const { id, path } = route.params;

	const location = useLibraryQuery(['locations.get', route.params.id]);

	const { data } = useLibraryQuery([
		'search.paths',
		{
			filters: [
				{
					filePath: {
						locations: { in: [id] },
						path: { path: path ?? '', location_id: id, include_descendants: false }
					}
				}
			],
			take: 100
		}
	]);
	const pathsItemsReferences = useMemo(() => data?.items ?? [], [data]);
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
				title: location.data?.name ?? 'Location'
			});
		}
	}, [location.data?.name, navigation, path]);

	useEffect(() => {
		getExplorerStore().locationId = id;
		getExplorerStore().path = path ?? '';
	}, [id, path]);

	return <Explorer items={pathsItems} />;
}
