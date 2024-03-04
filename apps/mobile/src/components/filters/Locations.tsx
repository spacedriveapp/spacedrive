import { MotiView } from 'moti';
import { FlatList, Pressable, Text, View } from 'react-native';
import { LinearTransition } from 'react-native-reanimated';
import { Location, useCache, useLibraryQuery, useNodes } from '@sd/client';
import { tw } from '~/lib/tailwind';
import { getSearchStore, useSearchStore } from '~/stores/searchStore';

import { Icon } from '../icons/Icon';
import Fade from '../layout/Fade';
import SectionTitle from '../layout/SectionTitle';
import VirtualizedListWrapper from '../layout/VirtualizedListWrapper';

export const Locations = () => {
	const locationsQuery = useLibraryQuery(['locations.list']);
	useNodes(locationsQuery.data?.nodes);
	const locations = useCache(locationsQuery.data?.items);

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
				<Fade color="mobile-screen" width={30} height="100%">
					<VirtualizedListWrapper horizontal>
						<FlatList
							data={locations}
							renderItem={({ item }) => (
								<Pressable
									onPress={() =>
										getSearchStore().updateFilters('locations', item.id)
									}
								>
									<LocationFilter data={item} />
								</Pressable>
							)}
							contentContainerStyle={tw`pl-6`}
							numColumns={locations && Math.ceil(Number(locations.length) / 2)}
							extraData={useSearchStore().filters}
							key={locations ? 'locationsSearch' : '_'}
							scrollEnabled={false}
							ItemSeparatorComponent={() => <View style={tw`w-2 h-2`} />}
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

const LocationFilter = ({ data }: Props) => {
	const isSelected = useSearchStore().isFilterSelected('locations', data.id);
	return (
		<MotiView
			animate={{
				borderColor: isSelected ? tw.color('accent') : tw.color('app-line/50')
			}}
			style={tw`mr-2 w-auto flex-row items-center gap-2 rounded-md border border-app-line/50 bg-app-box/50 p-2.5`}
		>
			<Icon size={20} name="Folder" />
			<Text style={tw`text-sm font-medium text-ink`}>{data.name}</Text>
		</MotiView>
	);
};

export default Locations;
