import { CaretRight, Pen, Trash } from 'phosphor-react-native';
import React from 'react';
import { Animated, FlatList, Text, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import { LibraryConfigWrapped, useBridgeQuery } from '@sd/client';
import { AnimatedButton } from '~/components/primitive/Button';
import DeleteLibraryDialog from '~/containers/dialog/DeleteLibraryDialog';
import tw from '~/lib/tailwind';
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
			containerStyle={tw.style(
				index !== 0 && 'mt-2',
				'bg-app-overlay border border-app-line rounded-lg px-4 py-3'
			)}
			enableTrackpadTwoFingerGesture
			renderRightActions={renderRightActions}
		>
			<View style={tw`flex flex-row items-center justify-between`}>
				<View>
					<Text style={tw`font-semibold text-ink`}>{library.config.name}</Text>
					<Text style={tw`mt-0.5 text-xs text-ink-dull`}>{library.uuid}</Text>
				</View>
				<CaretRight color={tw.color('ink-dull')} size={18} />
			</View>
		</Swipeable>
	);
}

const LibrarySettingsScreen = ({ navigation }: SettingsStackScreenProps<'LibrarySettings'>) => {
	const { data: libraries } = useBridgeQuery(['library.list']);

	return (
		<View style={tw`py-4 px-3 flex-1`}>
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
