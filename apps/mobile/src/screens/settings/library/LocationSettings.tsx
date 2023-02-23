import { CaretRight, Repeat, Trash } from 'phosphor-react-native';
import { Animated, FlatList, Pressable, Text, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import {
	Location,
	Node,
	arraysEqual,
	useLibraryMutation,
	useLibraryQuery,
	useOnlineLocations
} from '@sd/client';
import FolderIcon from '~/components/icons/FolderIcon';
import DeleteLocationModal from '~/components/modal/confirm-modals/DeleteLocationModal';
import { tw, twStyle } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

function LocationItem({ location, index }: { location: Location & { node: Node }; index: number }) {
	const fullRescan = useLibraryMutation('locations.fullRescan', {
		onMutate: () => {
			// TODO: Show Toast
		}
	});

	const onlineLocations = useOnlineLocations();

	const renderRightActions = (progress: Animated.AnimatedInterpolation<number>) => {
		const translate = progress.interpolate({
			inputRange: [0, 1],
			outputRange: [100, 0],
			extrapolate: 'clamp'
		});

		return (
			<Animated.View
				style={[tw`flex flex-row items-center`, { transform: [{ translateX: translate }] }]}
			>
				<DeleteLocationModal
					locationId={location.id}
					trigger={
						<View
							style={tw`bg-app-button border-app-line items-center justify-center rounded-md border py-1.5 px-3 shadow-sm`}
						>
							<Trash size={18} color="white" />
						</View>
					}
				/>
				{/* Full Re-scan IS too much here */}
				<Pressable
					style={tw`border-app-line bg-app-button mx-2 items-center justify-center rounded-md border py-1.5 px-3 shadow-sm`}
					onPress={() => fullRescan.mutate(location.id)}
				>
					<Repeat size={18} color="white" />
				</Pressable>
			</Animated.View>
		);
	};

	return (
		<Swipeable
			containerStyle={twStyle(
				'border-app-line bg-app-overlay rounded-lg border px-4 py-3',
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
							'absolute right-0 bottom-0.5 h-2 w-2 rounded-full',
							onlineLocations?.some((l) => arraysEqual(location.pub_id, l))
								? 'bg-green-500'
								: 'bg-red-500'
						)}
					/>
				</View>
				<View style={tw`mx-4 flex-1`}>
					<Text numberOfLines={1} style={tw`text-ink text-sm font-semibold`}>
						{location.name}
					</Text>
					<View style={tw`bg-app-highlight mt-0.5 self-start rounded py-[1px] px-1`}>
						<Text numberOfLines={1} style={tw`text-ink-dull text-xs font-semibold`}>
							{location.node.name}
						</Text>
					</View>
					<Text numberOfLines={1} style={tw`text-ink-dull mt-0.5 text-[10px] font-semibold`}>
						{location.path}
					</Text>
				</View>
				<CaretRight color={tw.color('ink-dull')} size={18} />
			</View>
		</Swipeable>
	);
}

// TODO: Add new location from here (ImportModal)

const LocationSettingsScreen = ({ navigation }: SettingsStackScreenProps<'LocationSettings'>) => {
	const { data: locations } = useLibraryQuery(['locations.list']);

	return (
		<View style={tw`flex-1 px-3 py-4`}>
			<FlatList
				data={locations}
				keyExtractor={(item) => item.id.toString()}
				renderItem={({ item, index }) => <LocationItem location={item} index={index} />}
			/>
		</View>
	);
};

export default LocationSettingsScreen;
