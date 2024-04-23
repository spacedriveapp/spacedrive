import { RouteProp } from '@react-navigation/native';
import { NativeStackNavigationProp } from '@react-navigation/native-stack';
import { useCache, useLibraryQuery, useNodes, usePathsExplorerQuery } from '@sd/client';
import { useEffect } from 'react';
import { useSharedValue } from 'react-native-reanimated';
import Explorer from '~/components/explorer/Explorer';
import { BrowseStackParamList } from '~/navigation/tabs/BrowseStack';
import { getExplorerStore } from '~/stores/explorerStore';

interface Props {
	route: RouteProp<BrowseStackParamList, 'Location'>;
	navigation: NativeStackNavigationProp<BrowseStackParamList, 'Location'>;
}

export default function LocationScreen({ navigation, route }: Props) {
	const { id, path } = route.params;
	const scrollY = useSharedValue(0);
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

	return (
			<Explorer headerKind='location' route={route} scrollY={scrollY} {...paths} />
	);
}
