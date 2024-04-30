import { DotsThreeOutlineVertical, Pen, Trash } from 'phosphor-react-native';
import React, { useEffect, useRef } from 'react';
import { Animated, FlatList, Pressable, Text, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import { LibraryConfigWrapped, useBridgeQuery } from '@sd/client';
import Fade from '~/components/layout/Fade';
import { ModalRef } from '~/components/layout/Modal';
import ScreenContainer from '~/components/layout/ScreenContainer';
import DeleteLibraryModal from '~/components/modal/confirmModals/DeleteLibraryModal';
import { AnimatedButton, FakeButton } from '~/components/primitive/Button';
import { tw, twStyle } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

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

	const swipeRef = useRef<Swipeable>(null);

	return (
		<Swipeable
			ref={swipeRef}
			containerStyle={twStyle(
				index !== 0 && 'mt-2',
				'rounded-lg border border-app-cardborder bg-app-card px-4 py-3'
			)}
			enableTrackpadTwoFingerGesture
			renderRightActions={renderRightActions}
		>
			<View style={tw`flex-row items-center justify-between`}>
				<View>
					<Text style={tw`text-md font-semibold text-ink`}>{library.config.name}</Text>
					<Text style={tw`mt-1 text-xs text-ink-dull`}>{library.uuid}</Text>
				</View>
				<Pressable onPress={() => swipeRef.current?.openRight()}>
					<DotsThreeOutlineVertical
						weight="fill"
						size={20}
						color={tw.color('ink-dull')}
					/>
				</Pressable>
			</View>
		</Swipeable>
	);
}

const LibrarySettingsScreen = ({ navigation }: SettingsStackScreenProps<'LibrarySettings'>) => {
	const libraryList = useBridgeQuery(['library.list']);
	const libraries = libraryList.data;

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
		<ScreenContainer style={tw`justify-start gap-0 px-6 py-0`} scrollview={false}>
			<Fade
				fadeSides="top-bottom"
				orientation="vertical"
				color="black"
				width={30}
				height="100%"
			>
				<FlatList
					data={libraries}
					contentContainerStyle={tw`py-5`}
					keyExtractor={(item) => item.uuid}
					renderItem={({ item, index }) => (
						<LibraryItem navigation={navigation} library={item} index={index} />
					)}
				/>
			</Fade>
		</ScreenContainer>
	);
};

export default LibrarySettingsScreen;
