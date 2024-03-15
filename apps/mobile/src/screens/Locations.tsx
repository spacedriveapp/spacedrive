import { useNavigation } from '@react-navigation/native';
import { DotsThreeOutlineVertical, Pen, Plus, Trash } from 'phosphor-react-native';
import { useMemo, useRef } from 'react';
import { Animated, FlatList, Pressable, Text, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
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
import { Icon } from '~/components/icons/Icon';
import Card from '~/components/layout/Card';
import Fade from '~/components/layout/Fade';
import { ModalRef } from '~/components/layout/Modal';
import ScreenContainer from '~/components/layout/ScreenContainer';
import DeleteLocationModal from '~/components/modal/confirmModals/DeleteLocationModal';
import ImportModal from '~/components/modal/ImportModal';
import { LocationModal } from '~/components/modal/location/LocationModal';
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
				style={tw`absolute z-10 items-center justify-center w-12 h-12 rounded-full bottom-7 right-7 bg-accent`}
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
					ListEmptyComponent={() => (
						<View style={tw`h-auto w-[85.5vw] flex-col items-center justify-center`}>
							<Icon name="Folder" size={90} />
							<Text style={tw`text-lg font-medium text-center text-ink-dull`}>
								You have no locations
							</Text>
						</View>
					)}
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

interface LocationItemProps {
	location: Location;
	onPress: () => void;
	editLocation: () => void;
	navigation: SettingsStackScreenProps<'LocationSettings'>['navigation'];
}

export const LocationItem = ({
	location,
	editLocation,
	onPress,
	navigation
}: LocationItemProps) => {
	const onlineLocations = useOnlineLocations();
	const online = onlineLocations.some((l) => arraysEqual(location.pub_id, l));
	const modalRef = useRef<ModalRef>(null);

	const renderRightActions = (
		progress: Animated.AnimatedInterpolation<number>,
		_: any,
		swipeable: Swipeable
	) => {
		const translate = progress.interpolate({
			inputRange: [0, 1],
			outputRange: [100, 0],
			extrapolate: 'clamp'
		});

		return (
			<Animated.View
				style={[
					tw`flex flex-row items-center gap-2 mr-3`,
					{ transform: [{ translateX: translate }] }
				]}
			>
				<Pressable
					style={tw`bg-mobile-button items-center justify-center rounded-md border border-mobile-lightborder px-3 py-1.5 shadow-sm`}
					onPress={() => {
						navigation.navigate('EditLocationSettings', { id: location.id });
						swipeable.close();
					}}
				>
					<Pen size={18} color="white" />
				</Pressable>
				<DeleteLocationModal
					locationId={location.id}
					trigger={
						<View
							style={tw`bg-mobile-button items-center justify-center rounded-md border border-mobile-lightborder px-3 py-1.5 shadow-sm`}
						>
							<Trash size={18} color="white" />
						</View>
					}
				/>
			</Animated.View>
		);
	};

	return (
		<Pressable onPress={onPress}>
			<Swipeable
				containerStyle={tw`border rounded-md border-mobile-cardborder bg-mobile-card`}
				enableTrackpadTwoFingerGesture
				renderRightActions={renderRightActions}
			>
				<Card style={tw`flex-row justify-between h-auto gap-3 p-3 border-0`}>
					<View style={tw`w-[50%] flex-row items-center gap-2`}>
						<View style={tw`relative`}>
							<FolderIcon size={38} />
							<View
								style={twStyle(
									'z-5 absolute bottom-[6px] right-[2px] h-2 w-2 rounded-full',
									online ? 'bg-green-500' : 'bg-red-500'
								)}
							/>
						</View>
						<View>
							<Text
								style={tw`w-auto max-w-[160px] text-sm font-bold text-white`}
								numberOfLines={1}
							>
								{location.name}
							</Text>
							<Text numberOfLines={1} style={tw`text-xs text-ink-dull`}>
								{location.path}
							</Text>
						</View>
					</View>
					<View style={tw`flex-row items-center gap-3`}>
						<View style={tw`bg-mobile-highlight rounded-md p-1.5`}>
							<Text
								style={tw`text-xs font-bold text-left text-ink-dull`}
								numberOfLines={1}
							>
								{`${byteSize(location.size_in_bytes)}`}
							</Text>
						</View>
						<Pressable onPress={() => modalRef.current?.present()}>
							<DotsThreeOutlineVertical
								weight="fill"
								size={20}
								color={tw.color('ink-dull')}
							/>
						</Pressable>
					</View>
				</Card>
			</Swipeable>
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
