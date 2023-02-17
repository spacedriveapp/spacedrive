import { CaretRight, Pen, Trash } from 'phosphor-react-native';
import React from 'react';
import { Animated, FlatList, Text, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import { LibraryConfigWrapped, useBridgeQuery } from '@sd/client';
import { AnimatedButton } from '~/components/primitive/Button';
import DeleteLibraryDialog from '~/containers/dialog/DeleteLibraryDialog';
import tw, { twStyle } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

function LibraryItem({
	library,
	index,
	navigation
}: {
	library: LibraryConfigWrapped;
	index: number;
	navigation: SettingsStackScreenProps<'LibrarySettings'>['navigation'];
}) {
	const renderRightActions = (
		progress: Animated.AnimatedInterpolation<number>,
		dragX: Animated.AnimatedInterpolation<number>
	) => {
		const translate = progress.interpolate({
			inputRange: [0, 1],
			outputRange: [100, 0],
			extrapolate: 'clamp'
		});

		return (
			<Animated.View
				style={[tw`flex flex-row items-center`, { transform: [{ translateX: translate }] }]}
			>
				<AnimatedButton size="md" onPress={() => navigation.replace('LibraryGeneralSettings')}>
					<Pen size={18} color="white" />
				</AnimatedButton>
				<DeleteLibraryDialog libraryUuid={library.uuid}>
					<AnimatedButton size="md" style={tw`mx-2`}>
						<Trash size={18} color="white" />
					</AnimatedButton>
				</DeleteLibraryDialog>
			</Animated.View>
		);
	};

	return (
		<Swipeable
			containerStyle={twStyle(
				index !== 0 && 'mt-2',
				'border-app-line bg-app-overlay rounded-lg border px-4 py-3'
			)}
			enableTrackpadTwoFingerGesture
			renderRightActions={renderRightActions}
		>
			<View style={tw`flex flex-row items-center justify-between`}>
				<View>
					<Text style={tw`text-ink font-semibold`}>{library.config.name}</Text>
					<Text style={tw`text-ink-dull mt-0.5 text-xs`}>{library.uuid}</Text>
				</View>
				<CaretRight color={tw.color('ink-dull')} size={18} />
			</View>
		</Swipeable>
	);
}

const LibrarySettingsScreen = ({ navigation }: SettingsStackScreenProps<'LibrarySettings'>) => {
	const { data: libraries } = useBridgeQuery(['library.list']);

	return (
		<View style={tw`flex-1 py-4 px-3`}>
			<FlatList
				data={libraries}
				keyExtractor={(item) => item.uuid}
				renderItem={({ item, index }) => (
					<LibraryItem navigation={navigation} library={item} index={index} />
				)}
			/>
		</View>
	);
};

export default LibrarySettingsScreen;
