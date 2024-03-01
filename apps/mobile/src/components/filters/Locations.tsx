import { MotiView, useDynamicAnimation } from 'moti';
import { FlatList, Text, View } from 'react-native';
import { Location, useCache, useLibraryQuery, useNodes } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';

import { Icon } from '../icons/Icon';
import Fade from '../layout/Fade';
import SectionTitle from '../layout/SectionTitle';
import VirtualizedListWrapper from '../layout/VirtualizedListWrapper';
import { Filters } from './FiltersList';

interface LocationsProps {
	selectedOptions: Partial<Filters[]>;
}

const Locations = ({ selectedOptions }: LocationsProps) => {
	const locationsQuery = useLibraryQuery(['locations.list']);
	useNodes(locationsQuery.data?.nodes);
	const locations = useCache(locationsQuery.data?.items);
	const layoutTransition = useDynamicAnimation(() => {
		return {
			translateY: 0
		};
	});
	return (
		<MotiView
			state={layoutTransition}
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
							renderItem={({ item }) => <LocationFilter data={item} />}
							contentContainerStyle={tw`pl-6`}
							numColumns={locations && Math.ceil(Number(locations.length) / 2)}
							key={locations ? 'locationsSearch' : '_'}
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

const LocationFilter = ({ data }: Props) => {
	return (
		<View
			style={tw`mr-2 w-auto flex-row items-center gap-2 rounded-md border border-app-line/50 bg-app-box/50 p-2.5`}
		>
			<Icon size={20} name="Folder" />
			<Text style={tw`text-sm font-medium text-ink-dull`}>{data.name}</Text>
		</View>
	);
};

export default Locations;
