import { useEffect } from 'react';
import { useLibraryQuery } from '@sd/client';
import Explorer from '~/components/explorer/Explorer';
import { SharedScreenProps } from '~/navigation/SharedScreens';
import { getExplorerStore } from '~/stores/explorerStore';

export default function LocationScreen({ navigation, route }: SharedScreenProps<'Location'>) {
	const { id, path } = route.params;

	const { data } = useLibraryQuery([
		'locations.getExplorerData',
		{
			location_id: id,
			path: path || '',
			limit: 100,
			cursor: null
		}
	]);

	useEffect(() => {
		// Set screen title to location.
		if (path && path !== '') {
			// Nested location.
			navigation.setOptions({
				title: path.split('/')[0]
			});
		} else {
			navigation.setOptions({
				title: data?.context.name
			});
		}
	}, [data, navigation, path]);

	useEffect(() => {
		getExplorerStore().locationId = id;
		getExplorerStore().path = path;
	}, [id, path]);

	return <Explorer data={data} />;
}
