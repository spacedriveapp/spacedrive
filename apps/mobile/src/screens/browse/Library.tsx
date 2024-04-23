import { FlatList, View } from 'react-native';
import { CATEGORIES_LIST } from '~/components/browse/BrowseCategories';
import LibraryItem from '~/components/browse/LibraryItem';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { tw } from '~/lib/tailwind';

export default function LibraryScreen() {
	return (
		<ScreenContainer style={tw`px-6 py-0`} scrollview={false}>
			<FlatList
				data={CATEGORIES_LIST}
				contentContainerStyle={tw`py-6`}
				keyExtractor={(item) => item.name}
				renderItem={({ item }) => (
					<LibraryItem viewStyle="list" icon={item.icon} name={item.name} />
				)}
				ItemSeparatorComponent={() => <View style={tw`h-2`} />}
				horizontal={false}
				numColumns={1}
			/>
		</ScreenContainer>
	);
}
