import { useEffect } from 'react';
import { useCache, useLibraryQuery, useNodes } from '@sd/client';
import Explorer from '~/components/explorer/Explorer';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';

export default function TagScreen({ navigation, route }: BrowseStackScreenProps<'Tag'>) {
	const { id } = route.params;

	const search = useLibraryQuery([
		'search.objects',
		{
			filters: [{ object: { tags: { in: [id] } } }],
			take: 100
		}
	]);
	useNodes(search.data?.nodes);
	const searchData = useCache(search.data?.items);

	const tag = useLibraryQuery(['tags.get', id]);
	useNodes(tag.data?.nodes);
	const tagData = useCache(tag.data?.item);

	useEffect(() => {
		// Set screen title to tag name.
		navigation.setOptions({
			title: tagData?.name ?? 'Tag'
		});
	}, [tagData?.name, navigation]);

	return <Explorer items={searchData} />;
}
