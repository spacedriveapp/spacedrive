import { MotiView } from 'moti';
import { memo, useCallback, useMemo } from 'react';
import { FlatList, Pressable, Text, View } from 'react-native';
import { LinearTransition } from 'react-native-reanimated';
import { Location, useLibraryQuery } from '@sd/client';
import { Icon } from '~/components/icons/Icon';
import Card from '~/components/layout/Card';
import Empty from '~/components/layout/Empty';
import Fade from '~/components/layout/Fade';
import SectionTitle from '~/components/layout/SectionTitle';
import VirtualizedListWrapper from '~/components/layout/VirtualizedListWrapper';
import { tw, twStyle } from '~/lib/tailwind';
import { useSearchStore } from '~/stores/searchStore';

const Locations = () => {
	const locationsQuery = useLibraryQuery(['locations.list']);
	const locations = locationsQuery.data;
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
				style="px-6 pb-3"
				title="Locations"
				sub="What locations should be searched?"
			/>
			<View>
				<Fade color="black" width={30} height="100%">
					<VirtualizedListWrapper contentContainerStyle={tw`px-6`} horizontal>
						<FlatList
							data={locations}
							renderItem={({ item }) => <LocationFilter data={item} />}
							numColumns={
								locations ? Math.max(Math.ceil(locations.length / 2), 2) : 1
							}
							ListEmptyComponent={
								<Empty
									icon="Folder"
									description="You have not added any locations"
								/>
							}
							extraData={searchStore.filters.locations}
							key={locations ? 'locationsSearch' : '_'}
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
	data: Location;
}

const LocationFilter = memo(({ data }: Props) => {
	const searchStore = useSearchStore();
	const isSelected = useMemo(
		() => searchStore.filters.locations.some((l) => l.id === data.id),
		[searchStore.filters.locations, data.id]
	);
	const onPress = useCallback(() => {
		searchStore.updateFilters('locations', {
			id: data.id,
			name: data.name as string
		});
	}, [data.id, data.name, searchStore]);

	return (
		<Pressable onPress={onPress}>
			<Card
				style={twStyle(`mr-2 w-auto flex-row items-center gap-2 p-2.5`, {
					borderColor: isSelected ? tw.color('accent') : tw.color('app-cardborder')
				})}
			>
				<Icon size={20} name="Folder" />
				<Text style={tw`text-sm font-medium text-ink`}>{data.name}</Text>
			</Card>
		</Pressable>
	);
});

export default Locations;
