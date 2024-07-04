import { MotiView } from 'moti';
import { memo, useCallback, useMemo } from 'react';
import { Pressable, Text, View } from 'react-native';
import { FlatList } from 'react-native-gesture-handler';
import { LinearTransition } from 'react-native-reanimated';
import { Tag, useLibraryQuery } from '@sd/client';
import Card from '~/components/layout/Card';
import Empty from '~/components/layout/Empty';
import Fade from '~/components/layout/Fade';
import SectionTitle from '~/components/layout/SectionTitle';
import VirtualizedListWrapper from '~/components/layout/VirtualizedListWrapper';
import { tw, twStyle } from '~/lib/tailwind';
import { useSearchStore } from '~/stores/searchStore';

const Tags = () => {
	const tags = useLibraryQuery(['tags.list']);
	const tagsData = tags.data;
	const searchStore = useSearchStore();

	return (
		<MotiView
			layout={LinearTransition.duration(300)}
			from={{ opacity: 0, translateY: 20 }}
			animate={{ opacity: 1, translateY: 0 }}
			transition={{ type: 'timing', duration: 300 }}
			exit={{ opacity: 0 }}
		>
			<SectionTitle
				style={tw`px-6 pb-3`}
				title="Tags"
				sub="What tags would you like to filter by?"
			/>
			<Fade color="black" width={30} height="100%">
				<VirtualizedListWrapper contentContainerStyle={tw`px-6`} horizontal>
					<FlatList
						data={tagsData}
						renderItem={({ item }) => <TagFilter tag={item} />}
						extraData={searchStore.filters.tags}
						alwaysBounceVertical={false}
						numColumns={tagsData ? Math.max(Math.ceil(tagsData.length / 2), 2) : 1}
						key={tagsData ? 'tagsSearch' : '_'}
						ListEmptyComponent={
							<Empty icon="Tags" description="You have not created any tags" />
						}
						ItemSeparatorComponent={() => <View style={tw`h-2 w-2`} />}
						keyExtractor={(item) => item.id.toString()}
						showsHorizontalScrollIndicator={false}
						style={tw`flex-row`}
					/>
				</VirtualizedListWrapper>
			</Fade>
		</MotiView>
	);
};

interface Props {
	tag: Tag;
}

const TagFilter = memo(({ tag }: Props) => {
	const searchStore = useSearchStore();
	const isSelected = useMemo(
		() => searchStore.filters.tags.some((filter) => filter.id === tag.id),
		[searchStore.filters.tags, tag]
	);
	const onPress = useCallback(() => {
		searchStore.updateFilters('tags', {
			id: tag.id,
			color: tag.color!
		});
	}, [searchStore, tag]);

	return (
		<Pressable onPress={onPress}>
			<Card
				style={twStyle(`mr-2 w-auto flex-row items-center gap-2 p-2.5`, {
					borderColor: isSelected ? tw.color('accent') : tw.color('app-cardborder')
				})}
			>
				<View
					style={twStyle(`h-3.5 w-3.5 rounded-full`, {
						backgroundColor: tag.color!
					})}
				/>
				<Text style={tw`text-sm font-medium text-ink`}>{tag?.name}</Text>
			</Card>
		</Pressable>
	);
});

export default Tags;
