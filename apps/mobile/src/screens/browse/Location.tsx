import { useEffect, useMemo } from 'react';
import { useLibraryQuery, useLibrarySubscription, usePathsExplorerQuery } from '@sd/client';
import Explorer from '~/components/explorer/Explorer';
import Empty from '~/components/layout/Empty';
import { useSortBy } from '~/hooks/useSortBy';
import { tw } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';
import { getExplorerStore } from '~/stores/explorerStore';

export default function LocationScreen({ navigation, route }: BrowseStackScreenProps<'Location'>) {
	const { id, path } = route.params;

	const location = useLibraryQuery(['locations.get', route.params.id]);
	const locationData = location.data;
	const order = useSortBy();
	const title = useMemo(() => {
		return path
			?.split('/')
			.filter((x) => x !== '')
			.pop();
	}, [path]);

	// makes sure that the location shows newest/modified objects
	// when a location is opened
	useLibrarySubscription(['locations.quickRescan', { sub_path: path ?? '', location_id: id }], {
		onData() {}
	});

	const paths = usePathsExplorerQuery({
		arg: {
			filters: [
				{ filePath: { hidden: false } },
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
		});
	}, [id, locationData?.name, navigation, path, title]);

	useEffect(() => {
		getExplorerStore().locationId = id;
		getExplorerStore().path = path ?? '';
	}, [id, path]);

	return (
		<Explorer
			isEmpty={paths.count === 0}
			emptyComponent={
				<Empty
					includeHeaderHeight
					icon={'FolderNoSpace'}
					style={tw`flex-1 items-center justify-center border-0`}
					iconSize={100}
					description={'No files found'}
				/>
			}
			{...paths}
		/>
	);
}
