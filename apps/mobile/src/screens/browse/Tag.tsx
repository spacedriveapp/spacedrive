import { useCache, useLibraryQuery, useNodes, useObjectsExplorerQuery } from '@sd/client';
import { useEffect } from 'react';
import { useSharedValue } from 'react-native-reanimated';
import Explorer from '~/components/explorer/Explorer';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';


interface Props {
	route: BrowseStackScreenProps<'Tag'>['route'];
	navigation: BrowseStackScreenProps<'Tag'>['navigation'];
}

export default function TagScreen({
	navigation,
	route,
}: Props) {
	const { id } = route.params;
	const scrollY = useSharedValue(0);
	const tag = useLibraryQuery(['tags.get', id]);
	useNodes(tag.data?.nodes);
	const tagData = useCache(tag.data?.item);

	const objects = useObjectsExplorerQuery({
		arg: { filters: [{ object: { tags: { in: [id] } } }], take: 30 },
		order: null
	});

	useEffect(() => {
		// Set screen title to tag name.
		navigation.setOptions({
			title: tagData?.name ?? 'Tag'
		});
	}, [tagData?.name, navigation]);

	return (
		 <Explorer headerKind='tag' route={route} scrollY={scrollY} {...objects} />
	);
}
