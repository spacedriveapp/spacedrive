import { useEffect } from 'react';
import { useCache, useLibraryQuery, useNodes, useObjectsExplorerQuery } from '@sd/client';
import Explorer from '~/components/explorer/Explorer';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';

export default function TagScreen({ navigation, route }: BrowseStackScreenProps<'Tag'>) {
	const { id } = route.params;

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

	return <Explorer {...objects} />;
}
