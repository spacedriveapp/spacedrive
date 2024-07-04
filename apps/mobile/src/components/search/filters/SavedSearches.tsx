import { useNavigation } from '@react-navigation/native';
import { MotiView } from 'moti';
import { MotiPressable } from 'moti/interactions';
import { X } from 'phosphor-react-native';
import { FlatList, Pressable, Text, View } from 'react-native';
import {
	SavedSearch as ISavedSearch,
	useLibraryMutation,
	useLibraryQuery,
	useRspcLibraryContext
} from '@sd/client';
import { Icon } from '~/components/icons/Icon';
import Card from '~/components/layout/Card';
import Empty from '~/components/layout/Empty';
import Fade from '~/components/layout/Fade';
import SectionTitle from '~/components/layout/SectionTitle';
import VirtualizedListWrapper from '~/components/layout/VirtualizedListWrapper';
import DottedDivider from '~/components/primitive/DottedDivider';
import { useSavedSearch } from '~/hooks/useSavedSearch';
import { tw } from '~/lib/tailwind';
import { getSearchStore } from '~/stores/searchStore';

const SavedSearches = () => {
	const { data: savedSearches } = useLibraryQuery(['search.saved.list']);
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
				<VirtualizedListWrapper contentContainerStyle={tw`w-full px-6`} horizontal>
					<FlatList
						data={savedSearches}
						ListEmptyComponent={() => {
							return (
								<Empty
									icon="Folder"
									description="No saved searches"
									style={tw`w-full`}
								/>
							);
						}}
						renderItem={({ item }) => <SavedSearch search={item} />}
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

interface Props {
	search: ISavedSearch;
}

const SavedSearch = ({ search }: Props) => {
	const navigation = useNavigation();
	const dataForSearch = useSavedSearch(search);
	const rspc = useRspcLibraryContext();
	const deleteSearch = useLibraryMutation('search.saved.delete', {
		onSuccess: () => rspc.queryClient.invalidateQueries(['search.saved.list'])
	});
	return (
		<MotiPressable
			from={{ opacity: 0, translateY: 20 }}
			animate={{ opacity: 1, translateY: 0 }}
			transition={{ type: 'timing', duration: 300 }}
			onPress={() => {
				getSearchStore().appliedFilters = dataForSearch;
				navigation.navigate('SearchStack', {
					screen: 'Search'
				});
			}}
		>
			<Card style={tw`mr-2 w-auto flex-row items-center gap-2 p-2.5`}>
				<Pressable onPress={async () => await deleteSearch.mutateAsync(search.id)}>
					<X size={14} color={tw.color('text-ink-dull')} />
				</Pressable>
				<Icon name="Folder" size={20} />
				<Text style={tw`text-sm font-medium text-ink`}>{search.name}</Text>
			</Card>
		</MotiPressable>
	);
};

export default SavedSearches;
