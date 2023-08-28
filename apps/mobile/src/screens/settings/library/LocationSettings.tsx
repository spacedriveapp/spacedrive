import { CaretRight, Pen, Repeat, Trash } from 'phosphor-react-native';
import { useEffect, useRef } from 'react';
import { Animated, FlatList, Pressable, Text, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import {
	Location,
	arraysEqual,
	useLibraryMutation,
	useLibraryQuery,
	useOnlineLocations
} from '@sd/client';
import FolderIcon from '~/components/icons/FolderIcon';
import { ModalRef } from '~/components/layout/Modal';
import ImportModal from '~/components/modal/ImportModal';
import DeleteLocationModal from '~/components/modal/confirmModals/DeleteLocationModal';
import { AnimatedButton } from '~/components/primitive/Button';
import { tw, twStyle } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

type LocationItemProps = {
	location: Location;
	index: number;
	navigation: SettingsStackScreenProps<'LocationSettings'>['navigation'];
};

function LocationItem({ location, index, navigation }: LocationItemProps) {
	const fullRescan = useLibraryMutation('locations.fullRescan', {
		onMutate: () => {
			// TODO: Show Toast
		}
	});

	const onlineLocations = useOnlineLocations();

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
					tw`mr-3 flex flex-row items-center gap-2`,
					{ transform: [{ translateX: translate }] }
				]}
			>
				<Pressable
					style={tw`items-center justify-center rounded-md border border-app-line bg-app-button px-3 py-1.5 shadow-sm`}
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
							style={tw`items-center justify-center rounded-md border border-app-line bg-app-button px-3 py-1.5 shadow-sm`}
						>
							<Trash size={18} color="white" />
						</View>
					}
				/>
				{/* Full Re-scan IS too much here */}
				<Pressable
					style={tw`items-center justify-center rounded-md border border-app-line bg-app-button px-3 py-1.5 shadow-sm`}
					onPress={() =>
						fullRescan.mutate({ location_id: location.id, reidentify_objects: true })
					}
				>
					<Repeat size={18} color="white" />
				</Pressable>
			</Animated.View>
		);
	};

	return (
		<Swipeable
			containerStyle={twStyle(
				'rounded-lg border border-app-line bg-app-overlay px-4 py-3',
				index !== 0 && 'mt-2'
			)}
			enableTrackpadTwoFingerGesture
			renderRightActions={renderRightActions}
		>
			<View style={tw`flex flex-row items-center`}>
				<View style={tw`relative`}>
					<FolderIcon size={32} />
					{/* Online/Offline Indicator */}
					<View
						style={twStyle(
							'absolute bottom-0.5 right-0 h-2 w-2 rounded-full',
							onlineLocations.some((l) => arraysEqual(location.pub_id, l))
								? 'bg-green-500'
								: 'bg-red-500'
						)}
					/>
				</View>
				<View style={tw`mx-4 flex-1`}>
					<Text numberOfLines={1} style={tw`text-sm font-semibold text-ink`}>
						{location.name}
					</Text>
					{/* // TODO: This is ephemeral so it should not come from the DB. Eg. a external USB can move between nodes */}
					{/* {location.node && (
						<View style={tw`mt-0.5 self-start rounded bg-app-highlight px-1 py-[1px]`}>
							<Text numberOfLines={1} style={tw`text-xs font-semibold text-ink-dull`}>
								{location.node.name}
							</Text>
						</View>
					)} */}
					<Text
						numberOfLines={1}
						style={tw`mt-0.5 text-[10px] font-semibold text-ink-dull`}
					>
						{location.path}
					</Text>
				</View>
				<CaretRight color={tw.color('ink-dull')} size={18} />
			</View>
		</Swipeable>
	);
}

const LocationSettingsScreen = ({ navigation }: SettingsStackScreenProps<'LocationSettings'>) => {
	const { data: locations } = useLibraryQuery(['locations.list']);

	useEffect(() => {
		navigation.setOptions({
			headerRight: () => (
				<AnimatedButton
					variant="accent"
					style={tw`mr-2`}
					size="sm"
					onPress={() => modalRef.current?.present()}
				>
					<Text style={tw`text-white`}>New</Text>
				</AnimatedButton>
			)
		});
	}, [navigation]);

	const modalRef = useRef<ModalRef>(null);

	return (
		<View style={tw`flex-1 px-3 py-4`}>
			<FlatList
				data={locations}
				keyExtractor={(item) => item.id.toString()}
				renderItem={({ item, index }) => (
					<LocationItem navigation={navigation} location={item} index={index} />
				)}
			/>
			<ImportModal ref={modalRef} />
		</View>
	);
};

export default LocationSettingsScreen;
