import React, { useEffect } from 'react';
import { useLibraryQuery } from '@sd/client';
import Explorer from '~/components/explorer/Explorer';
import { SharedScreenProps } from '~/navigation/SharedScreens';
import { getExplorerStore } from '~/stores/explorerStore';

export default function LocationScreen({ navigation, route }: SharedScreenProps<'Location'>) {
	const { id, path } = route.params;

	const location = useLibraryQuery(['locations.get', route.params.id]);

	const { data } = useLibraryQuery([
		'search.paths',
		{
			filter: {
				locationId: id,
				path: path ?? ''
			},
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
				title: location.data?.name ?? 'Location',
			});
		}
	}, [location.data?.name, navigation, path]);

	useEffect(() => {
		getExplorerStore().locationId = id;
		getExplorerStore().path = path ?? '';
	}, [id, path]);

	return <Explorer items={data?.items} />;
}
