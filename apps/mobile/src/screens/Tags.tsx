import { useNavigation } from '@react-navigation/native';
import { View } from 'react-native';
import { FlatList } from 'react-native-gesture-handler';
import { useCache, useLibraryQuery, useNodes } from '@sd/client';
import { BrowseTagItem } from '~/components/browse/BrowseTags';
import Fade from '~/components/layout/Fade';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { tw } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';

export default function Tags() {
	const tags = useLibraryQuery(['tags.list']);
	const navigation = useNavigation<BrowseStackScreenProps<'Browse'>['navigation']>();

	useNodes(tags.data?.nodes);
	const tagData = useCache(tags.data?.items);
	return (
		<ScreenContainer scrollview={false} style={tw`relative px-7 py-0`}>
			<Fade
				fadeSides="top-bottom"
				orientation="vertical"
				color="mobile-screen"
				width={30}
				height="100%"
			>
				<FlatList
					data={tagData}
					renderItem={({ item }) => (
						<BrowseTagItem
							tagStyle="w-[105px]"
							tag={item}
							onPress={() =>
								navigation.navigate('Tag', { id: item.id, color: item.color! })
							}
						/>
					)}
					numColumns={3}
					columnWrapperStyle={tw`gap-2.5`}
					horizontal={false}
					keyExtractor={(item) => item.id.toString()}
					showsHorizontalScrollIndicator={false}
					ItemSeparatorComponent={() => <View style={tw`h-2.5`} />}
					contentContainerStyle={tw`py-5`}
				/>
			</Fade>
		</ScreenContainer>
	);
}
