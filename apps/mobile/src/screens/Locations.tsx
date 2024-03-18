import { useNavigation } from '@react-navigation/native';
import { Plus } from 'phosphor-react-native';
import { useMemo, useRef } from 'react';
import { FlatList, Pressable, View } from 'react-native';
import { useDebounce } from 'use-debounce';
import { useCache, useLibraryQuery, useNodes } from '@sd/client';
import Empty from '~/components/layout/Empty';
import Fade from '~/components/layout/Fade';
import { ModalRef } from '~/components/layout/Modal';
import ScreenContainer from '~/components/layout/ScreenContainer';
import LocationItem from '~/components/locations/LocationItem';
import ImportModal from '~/components/modal/ImportModal';
import { tw, twStyle } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';
import { useSearchStore } from '~/stores/searchStore';

interface Props {
	redirectToLocationSettings?: boolean;
}

export const Locations = ({ redirectToLocationSettings }: Props) => {
	const locationsQuery = useLibraryQuery(['locations.list']);
	useNodes(locationsQuery.data?.nodes);
	const locations = useCache(locationsQuery.data?.items);
	const { search } = useSearchStore();
	const modalRef = useRef<ModalRef>(null);
	const [debouncedSearch] = useDebounce(search, 200);
	const filteredLocations = useMemo(
		() =>
			locations?.filter((location) =>
				location.name?.toLowerCase().includes(debouncedSearch.toLowerCase())
			) ?? [],
		[debouncedSearch, locations]
	);

	const navigation = useNavigation<
		BrowseStackScreenProps<'Browse'>['navigation'] &
			SettingsStackScreenProps<'Settings'>['navigation']
	>();
	return (
		<ScreenContainer scrollview={false} style={tw`relative px-6 py-0`}>
			<Pressable
				style={tw`absolute bottom-7 right-7 z-10 h-12 w-12 items-center justify-center rounded-full bg-accent`}
				onPress={() => {
					modalRef.current?.present();
				}}
			>
				<Plus size={20} weight="bold" style={tw`text-ink`} />
			</Pressable>

			<Fade
				fadeSides="top-bottom"
				orientation="vertical"
				color="black"
				width={30}
				height="100%"
			>
				<FlatList
					data={filteredLocations}
					contentContainerStyle={twStyle(
						`py-6`,
						filteredLocations.length === 0 && 'h-full items-center justify-center'
					)}
					keyExtractor={(location) => location.id.toString()}
					ItemSeparatorComponent={() => <View style={tw`h-2`} />}
					showsVerticalScrollIndicator={false}
					scrollEnabled={filteredLocations.length > 0}
					ListEmptyComponent={
						<Empty
							icon="Folder"
							style={'border-0'}
							textSize="text-md"
							iconSize={84}
							description="You have not added any locations"
						/>
					}
					renderItem={({ item }) => (
						<LocationItem
							navigation={navigation}
							editLocation={() =>
								navigation.navigate('SettingsStack', {
									screen: 'EditLocationSettings',
									params: { id: item.id }
								})
							}
							onPress={() => {
								if (redirectToLocationSettings) {
									navigation.navigate('SettingsStack', {
										screen: 'EditLocationSettings',
										params: { id: item.id }
									});
								} else {
									navigation.navigate('BrowseStack', {
										screen: 'Location',
										params: { id: item.id }
									});
								}
							}}
							location={item}
						/>
					)}
				/>
			</Fade>
			<ImportModal ref={modalRef} />
		</ScreenContainer>
	);
};
