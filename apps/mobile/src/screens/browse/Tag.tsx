import { useLibraryQuery, useObjectsExplorerQuery } from '@sd/client';
import { useEffect } from 'react';
import Explorer from '~/components/explorer/Explorer';
import Empty from '~/components/layout/Empty';
import { tw } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';

export default function TagScreen({ navigation, route }: BrowseStackScreenProps<'Tag'>) {
	const { id } = route.params;

	const tag = useLibraryQuery(['tags.get', id]);
	const tagData = tag.data;

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

	return <Explorer
		isEmpty={objects.count === 0}
		emptyComponent={<Empty
		includeHeaderHeight
		icon={'Tags'}
		style={tw`flex-1 items-center justify-center border-0`}
		textSize="text-md"
		iconSize={100}
		description={'No items assigned to this tag'}
	/>} {...objects} />;
}
