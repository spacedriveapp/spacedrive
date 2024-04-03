import { MotiView } from 'moti';
import { MotiPressable } from 'moti/interactions';
import { FlatList, Text, View } from 'react-native';
import { Icon } from '~/components/icons/Icon';
import Card from '~/components/layout/Card';
import Fade from '~/components/layout/Fade';
import SectionTitle from '~/components/layout/SectionTitle';
import VirtualizedListWrapper from '~/components/layout/VirtualizedListWrapper';
import DottedDivider from '~/components/primitive/DottedDivider';
import { tw } from '~/lib/tailwind';

const SavedSearches = () => {
	return (
		<Fade color="black" width={30} height="100%">
			<MotiView
				from={{ opacity: 0, translateY: 20 }}
				animate={{ opacity: 1, translateY: 0 }}
				transition={{ type: 'timing', duration: 300 }}
			>
				<SectionTitle
					style={tw`px-6 pb-3`}
					title="Saved searches"
					sub="Tap a saved search for searching quickly"
				/>
				<VirtualizedListWrapper contentContainerStyle={tw`px-6`} horizontal>
					<FlatList
						data={Array.from({ length: 6 })}
						renderItem={() => <SavedSearch />}
						keyExtractor={(_, index) => index.toString()}
						numColumns={Math.ceil(6 / 2)}
						scrollEnabled={false}
						contentContainerStyle={tw`w-full`}
						showsHorizontalScrollIndicator={false}
						style={tw`flex-row`}
						ItemSeparatorComponent={() => <View style={tw`h-2 w-2`} />}
					/>
				</VirtualizedListWrapper>
				<DottedDivider style={'mt-6'} />
			</MotiView>
		</Fade>
	);
};

const SavedSearch = () => {
	return (
		<MotiPressable
			from={{ opacity: 0, translateY: 20 }}
			animate={{ opacity: 1, translateY: 0 }}
			transition={{ type: 'timing', duration: 300 }}
		>
			<Card style={tw`mr-2 w-auto flex-row gap-2 p-2.5`}>
				<Icon name="Folder" size={20} />
				<Text style={tw`text-sm font-medium text-ink`}>Saved search</Text>
			</Card>
		</MotiPressable>
	);
};

export default SavedSearches;
