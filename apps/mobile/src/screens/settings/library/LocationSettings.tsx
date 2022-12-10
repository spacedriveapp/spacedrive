import { Location, Node, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { CaretRight, Repeat, Trash } from 'phosphor-react-native';
import { Animated, FlatList, Pressable, Text, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import FolderIcon from '~/components/icons/FolderIcon';
import DeleteLocationDialog from '~/containers/dialog/DeleteLocationDialog';
import tw from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

function LocationItem({ location, index }: { location: Location & { node: Node }; index: number }) {
	const { mutate: fullRescan } = useLibraryMutation('locations.fullRescan', {
		onMutate: () => {
			// TODO: Show Toast
		}
	});

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
				<DeleteLocationDialog locationId={location.id}>
					<View
						style={tw`py-1.5 px-3 bg-app-button border-app-line border rounded-md items-center justify-center shadow-sm`}
					>
						<Trash size={18} color="white" />
					</View>
				</DeleteLocationDialog>
				{/* Full Re-scan IS too much here */}
				<Pressable
					style={tw`py-1.5 px-3 bg-app-button border-app-line border rounded-md items-center justify-center shadow-sm mx-2`}
					onPress={() => fullRescan(location.id)}
				>
					<Repeat size={18} color="white" />
				</Pressable>
			</Animated.View>
		);
	};

	return (
		<Swipeable
			containerStyle={tw`bg-app-overlay border border-app-line rounded-lg`}
			enableTrackpadTwoFingerGesture
			renderRightActions={renderRightActions}
		>
			<View style={tw.style('px-4 py-3', index !== 0 && 'mt-2')}>
				<View style={tw`flex flex-row items-center`}>
					<View style={tw`relative`}>
						<FolderIcon size={32} />
						{/* Online/Offline Indicator */}
						<View
							style={tw.style(
								'absolute w-2 h-2 right-0 bottom-0.5 rounded-full',
								location.is_online ? 'bg-green-500' : 'bg-red-500'
							)}
						/>
					</View>
					<View style={tw`flex-1 mx-4`}>
						<Text numberOfLines={1} style={tw`text-sm font-semibold text-ink`}>
							{location.name}
						</Text>
						<View style={tw`self-start bg-app-highlight py-[1px] px-1 rounded mt-0.5`}>
							<Text numberOfLines={1} style={tw`text-xs font-semibold text-ink-dull`}>
								{location.node.name}
							</Text>
						</View>
						<Text numberOfLines={1} style={tw`mt-0.5 text-[10px] font-semibold text-ink-dull`}>
							{location.local_path}
						</Text>
					</View>
					<CaretRight color={tw.color('ink-dull')} size={18} />
				</View>
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
