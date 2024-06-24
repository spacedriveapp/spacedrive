import { LibraryConfigWrapped, useBridgeQuery, useLibraryContext } from '@sd/client';
import { DotsThreeOutlineVertical, Pen, Trash } from 'phosphor-react-native';
import React, { useEffect, useRef } from 'react';
import { Animated, FlatList, Pressable, Text, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import { ModalRef } from '~/components/layout/Modal';
import ScreenContainer from '~/components/layout/ScreenContainer';
import DeleteLibraryModal from '~/components/modal/confirmModals/DeleteLibraryModal';
import { AnimatedButton, FakeButton } from '~/components/primitive/Button';
import { tw, twStyle } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

function LibraryItem({
	library,
	index,
	navigation,
	current
}: {
	library: LibraryConfigWrapped;
	index: number;
	current: boolean;
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
					<View style={tw`flex-row items-center gap-2`}>
					<Text style={tw`text-md font-semibold text-ink`}>{library.config.name}</Text>
					{current && (
						<View style={tw`rounded-md bg-accent px-1.5 py-[2px]`}>
							<Text style={tw`text-xs font-semibold text-white`}>Current</Text>
						</View>
					)}
					</View>
					<Text style={tw`mt-1.5 text-xs text-ink-dull`}>{library.uuid}</Text>
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
	const { library } = useLibraryContext();

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
				<FlatList
					data={libraries}
					contentContainerStyle={tw`py-5`}
					keyExtractor={(item) => item.uuid}
					renderItem={({ item, index }) => (
						<LibraryItem
						 current={item.uuid === library.uuid}
						 navigation={navigation}
						 library={item}
						 index={index}
						  />
					)}
				/>
		</ScreenContainer>
	);
};

export default LibrarySettingsScreen;
