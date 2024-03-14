import { MotiView } from 'moti';
import { memo, useCallback, useMemo } from 'react';
import { Pressable, Text, View } from 'react-native';
import { FlatList } from 'react-native-gesture-handler';
import { LinearTransition } from 'react-native-reanimated';
import { Tag, useCache, useLibraryQuery, useNodes } from '@sd/client';
import Fade from '~/components/layout/Fade';
import SectionTitle from '~/components/layout/SectionTitle';
import VirtualizedListWrapper from '~/components/layout/VirtualizedListWrapper';
import { tw, twStyle } from '~/lib/tailwind';
import { useSearchStore } from '~/stores/searchStore';

const Tags = () => {
	const tags = useLibraryQuery(['tags.list']);
	useNodes(tags.data?.nodes);
	const tagsData = useCache(tags.data?.items);
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
			<View>
				<Fade color="mobile-screen" width={30} height="100%">
					<VirtualizedListWrapper horizontal>
						<FlatList
							data={tagsData}
							renderItem={({ item }) => <TagFilter tag={item} />}
							contentContainerStyle={tw`pl-6`}
							extraData={searchStore.filters.tags}
							numColumns={tagsData && Math.ceil(Number(tagsData.length ?? 0) / 2)}
							key={tagsData ? 'tagsSearch' : '_'}
							scrollEnabled={false}
							ItemSeparatorComponent={() => <View style={tw`h-2 w-2`} />}
							keyExtractor={(item) => item.id.toString()}
							showsHorizontalScrollIndicator={false}
							style={tw`flex-row`}
						/>
					</VirtualizedListWrapper>
				</Fade>
			</View>
		</MotiView>
	);
};

interface Props {
	tag: Tag;
}

const TagFilter = memo(({ tag }: Props) => {
	const searchStore = useSearchStore();
	const isSelected = useMemo(
		() =>
			searchStore.filters.tags.some(
				(filter) => filter.id === tag.id && filter.color === tag.color
			),
		[searchStore.filters.tags, tag]
	);
	const onPress = useCallback(() => {
		searchStore.updateFilters('tags', {
			id: tag.id,
			color: tag.color!
		});
	}, [searchStore, tag.id, tag.color]);
	return (
		<Pressable
			onPress={onPress}
			style={twStyle(
				`mr-2 w-auto flex-row items-center gap-2 rounded-md border border-app-line/50 bg-app-box/50 p-2.5`,
				{
					borderColor: isSelected ? tw.color('accent') : tw.color('app-line/50')
				}
			)}
		>
			<View
				style={twStyle(`h-5 w-5 rounded-full`, {
					backgroundColor: tag.color!
				})}
			/>
			<Text style={tw`text-sm font-medium text-ink`}>{tag?.name}</Text>
		</Pressable>
	);
});

export default Tags;
