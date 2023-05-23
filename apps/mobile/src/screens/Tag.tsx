import { useEffect } from 'react';
import { useLibraryQuery } from '@sd/client';
import Explorer from '~/components/explorer/Explorer';
import { SharedScreenProps } from '~/navigation/SharedScreens';

export default function TagScreen({ navigation, route }: SharedScreenProps<'Tag'>) {
	const { id } = route.params;

	const search = useLibraryQuery([
		'search.objects',
		{
			filter: {
				tags: [id]
			}
		}
	]);

	const tag = useLibraryQuery(['tags.get', id]);

	useEffect(() => {
		// Set screen title to tag name.
		navigation.setOptions({
			title: tag.data?.name ?? 'Tag'
		});
	}, [tag.data?.name, navigation]);

	useEffect(() => {
		// no location, this is tags!
		// getExplorerStore().locationId = id;
		// getExplorerStore().path = path;
	}, [id]);

	return <Explorer items={search.data?.items} />;
}
