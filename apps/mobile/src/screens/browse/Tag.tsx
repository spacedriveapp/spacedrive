import { useEffect } from 'react';
import { useLibraryQuery, usePathsExplorerQuery } from '@sd/client';
import Explorer from '~/components/explorer/Explorer';
import Empty from '~/components/layout/Empty';
import { tw } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';

export default function TagScreen({ navigation, route }: BrowseStackScreenProps<'Tag'>) {
	const { id } = route.params;

	const tag = useLibraryQuery(['tags.get', id]);
	const tagData = tag.data;

	const objects = usePathsExplorerQuery({
		arg: { filters: [{ object: { tags: { in: [id] } } }], take: 30 },
		enabled: typeof id === 'number',
		order: null
	});

	useEffect(() => {
		// Set screen title to tag name.
		if (tagData) {
			navigation.setParams({
				id: tagData.id,
				color: tagData.color as string
			});
			navigation.setOptions({
				title: tagData.name ?? 'Tag'
			});
		}
	}, [tagData, id, navigation]);

	return (
		<Explorer
			isEmpty={objects.count === 0}
			emptyComponent={
				<Empty
					includeHeaderHeight
					icon={'Tags'}
					style={tw`flex-1 items-center justify-center border-0`}
					iconSize={80}
					description={'No items assigned to this tag'}
				/>
			}
			{...objects}
		/>
	);
}
