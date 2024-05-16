import { useLibraryQuery, usePathsExplorerQuery } from '@sd/client';
import { useEffect, useMemo } from 'react';
import Explorer from '~/components/explorer/Explorer';
import { useSortBy } from '~/hooks/useSortBy';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';
import { getExplorerStore } from '~/stores/explorerStore';

export default function LocationScreen({ navigation, route }: BrowseStackScreenProps<'Location'>) {
	const { id, path } = route.params;

	const location = useLibraryQuery(['locations.get', route.params.id]);
	const locationData = location.data;
	const order = useSortBy();
	const title = useMemo(() => {
	return path?.split('/')
	.filter((x) => x !== '')
	.pop();
	}, [path])

	const paths = usePathsExplorerQuery({
		arg: {
			filters: [
				{ filePath: { hidden: false }},
				{ filePath: { locations: { in: [id] } } },
				{
					filePath: {
						path: {
							location_id: id,
							path: path ?? '',
							include_descendants: false
						}
					}
				}
			].filter(Boolean) as any,
			take: 30
		},
		order,
		onSuccess: () => getExplorerStore().resetNewThumbnails()
	});

	useEffect(() => {
		// Set screen title to location.
		if (path && path !== '') {
			// Nested location.
			navigation.setOptions({
				title
			});
		} else {
			navigation.setOptions({
				title: locationData?.name ?? 'Location'
			});
		}
		// sets params for handling when clicking on search within header
		navigation.setParams({
			id: id,
			name: locationData?.name ?? 'Location'
		})
	}, [id, locationData?.name, navigation, path, title]);

	useEffect(() => {
		getExplorerStore().locationId = id;
		getExplorerStore().path = path ?? '';
	}, [id, path]);

	return <Explorer {...paths} />;
}
