import { useEffect } from 'react';
import { useLibraryQuery } from '@sd/client';
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

	const tag = useLibraryQuery(['tags.get', id]);

	useEffect(() => {
		// Set screen title to tag name.
		navigation.setOptions({
			title: tag.data?.name ?? 'Tag'
		});
	}, [tag.data?.name, navigation]);

	return <Explorer items={search.data?.items} />;
}
