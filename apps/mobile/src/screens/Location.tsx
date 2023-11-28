import React, { useEffect } from 'react';
import { useLibraryQuery } from '@sd/client';
import Explorer from '~/components/explorer/Explorer';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';
import { getExplorerStore } from '~/stores/explorerStore';

export default function LocationScreen({ navigation, route }: BrowseStackScreenProps<'Location'>) {
	const { id, path } = route.params;

	const location = useLibraryQuery(['locations.get', id]);

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

	return <Explorer items={data?.items} />;
}
