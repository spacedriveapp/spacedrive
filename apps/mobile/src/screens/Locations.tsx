import { useBottomTabBarHeight } from '@react-navigation/bottom-tabs';
import { useNavigation } from '@react-navigation/native';
import { DotsThreeOutlineVertical } from 'phosphor-react-native';
import { useMemo, useRef } from 'react';
import { FlatList, Pressable, Text, View } from 'react-native';
import { useDebounce } from 'use-debounce';
import {
	arraysEqual,
	byteSize,
	Location,
	useCache,
	useLibraryQuery,
	useNodes,
	useOnlineLocations
} from '@sd/client';
import FolderIcon from '~/components/icons/FolderIcon';
import Fade from '~/components/layout/Fade';
import { ModalRef } from '~/components/layout/Modal';
import { LocationModal } from '~/components/modal/location/LocationModal';
import { tw, twStyle } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';
import { useSearchStore } from '~/stores/searchStore';

export const Locations = () => {
	const locationsQuery = useLibraryQuery(['locations.list']);
	useNodes(locationsQuery.data?.nodes);
	const locations = useCache(locationsQuery.data?.items);
	const { search } = useSearchStore();
	const [debouncedSearch] = useDebounce(search, 200);
	const filteredLocations = useMemo(
		() =>
			locations?.filter((location) =>
				location.name?.toLowerCase().includes(debouncedSearch.toLowerCase())
			) ?? [],
		[debouncedSearch, locations]
	);

	const height = useBottomTabBarHeight();

	const navigation = useNavigation<
		BrowseStackScreenProps<'Browse'>['navigation'] &
			SettingsStackScreenProps<'Settings'>['navigation']
	>();
	return (
		<View style={twStyle('relative flex-1 bg-mobile-screen px-7', { marginBottom: height })}>
			<Fade
				fadeSides="top-bottom"
				orientation="vertical"
				color="mobile-screen"
				width={50}
				height="100%"
			>
				<FlatList
					data={filteredLocations}
					contentContainerStyle={tw`py-5`}
					keyExtractor={(location) => location.id.toString()}
					ItemSeparatorComponent={() => <View style={tw`h-2`} />}
					showsVerticalScrollIndicator={false}
					renderItem={({ item }) => (
						<LocationItem
							editLocation={() =>
								navigation.navigate('SettingsStack', {
									screen: 'EditLocationSettings',
									params: { id: item.id }
								})
							}
							onPress={() => navigation.navigate('Location', { id: item.id })}
							location={item}
						/>
					)}
				/>
			</Fade>
		</View>
	);
};

interface LocationItemProps {
	location: Location;
	onPress: () => void;
	editLocation: () => void;
}

const LocationItem: React.FC<LocationItemProps> = ({
	location,
	editLocation,
	onPress
}: LocationItemProps) => {
	const onlineLocations = useOnlineLocations();
	const online = onlineLocations.some((l) => arraysEqual(location.pub_id, l));
	const modalRef = useRef<ModalRef>(null);
	return (
		<Pressable onPress={onPress}>
			<View
				style={tw`h-fit w-full flex-row justify-between gap-3 rounded-md border border-sidebar-line/50 bg-sidebar-box p-2`}
			>
				<View style={tw`flex-row items-center gap-2`}>
					<View style={tw`relative`}>
						<FolderIcon size={42} />
						<View
							style={twStyle(
								'z-5 absolute bottom-[6px] right-[2px] size-2 rounded-full',
								online ? 'bg-green-500' : 'bg-red-500'
							)}
						/>
					</View>
					<Text
						style={tw`w-fit max-w-[160px] truncate text-sm font-bold text-white`}
						numberOfLines={1}
					>
						{location.name}
					</Text>
				</View>
				<View style={tw`flex-row items-center gap-3`}>
					<View style={tw`rounded-md bg-app-input p-1.5`}>
						<Text
							style={tw`truncate text-left text-xs font-bold text-ink-dull`}
							numberOfLines={1}
						>
							{`${byteSize(location.size_in_bytes)}`}
						</Text>
					</View>
					<Pressable onPress={() => modalRef.current?.present()}>
						<DotsThreeOutlineVertical
							weight="fill"
							size={20}
							color={tw.color('ink-faint')}
						/>
					</Pressable>
				</View>
			</View>
			<LocationModal
				editLocation={() => {
					editLocation();
					modalRef.current?.close();
				}}
				locationId={location.id}
				ref={modalRef}
			/>
		</Pressable>
	);
};
