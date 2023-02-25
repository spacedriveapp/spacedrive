import { useEffect } from 'react';
import { useLibraryQuery } from '@sd/client';
import Explorer from '~/components/explorer/Explorer';
import { SharedScreenProps } from '~/navigation/SharedScreens';
import { getExplorerStore } from '~/stores/explorerStore';

export default function TagScreen({ navigation, route }: SharedScreenProps<'Tag'>) {
	const { id } = route.params;

	const { data } = useLibraryQuery(['tags.getExplorerData', id]);

	useEffect(() => {
		// Set screen title to tag name.
		navigation.setOptions({
			title: data?.context.name ?? 'Tag'
		});
	}, [data?.context.name, navigation]);

	useEffect(() => {
		getExplorerStore().locationId = id;
		// getExplorerStore().path = path;
	}, [id]);

	return <Explorer data={data} />;
}
