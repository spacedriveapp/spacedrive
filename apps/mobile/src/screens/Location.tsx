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
		// Not sure why we do this.
		getExplorerStore().locationId = id;
	}, [id]);

	return <Explorer data={data} />;
}
