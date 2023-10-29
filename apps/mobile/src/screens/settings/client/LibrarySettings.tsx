import { CaretRight, Pen, Trash } from 'phosphor-react-native';
import React, { useEffect, useRef } from 'react';
import { Animated, FlatList, Text, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import { LibraryConfigWrapped, useBridgeQuery } from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';
import DeleteLibraryModal from '~/components/modal/confirmModals/DeleteLibraryModal';
import { AnimatedButton, FakeButton } from '~/components/primitive/Button';
import { tw, twStyle } from '~/lib/tailwind';
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
				<AnimatedButton onPress={() => navigation.replace('LibraryGeneralSettings')}>
					<Pen size={18} color="white" />
				</AnimatedButton>
				<DeleteLibraryModal
					libraryUuid={library.uuid}
					trigger={
						<FakeButton style={tw`mx-2`}>
							<Trash size={18} color="white" />
						</FakeButton>
					}
				/>
			</Animated.View>
		);
	};

	return (
		<Swipeable
			containerStyle={twStyle(
				index !== 0 && 'mt-2',
				'rounded-lg border border-app-line bg-app-overlay px-4 py-3'
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
