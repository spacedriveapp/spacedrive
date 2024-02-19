import { useNavigation } from '@react-navigation/native';
import { Tag, useCache, useLibraryQuery, useNodes } from '@sd/client';
import { DotsThreeOutlineVertical, Eye, Pen, Plus, Trash } from 'phosphor-react-native';
import React, { useRef } from 'react';
import { Animated, Pressable, Text, View } from 'react-native';
import { FlatList, Swipeable } from 'react-native-gesture-handler';
import { ClassInput } from 'twrnc/dist/esm/types';
import { ModalRef } from '~/components/layout/Modal';
import { tw, twStyle } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';

import { Icon } from '../icons/Icon';
import Fade from '../layout/Fade';
import DeleteTagModal from '../modal/confirmModals/DeleteTagModal';
import CreateTagModal from '../modal/tag/CreateTagModal';
import { TagModal } from '../modal/tag/TagModal';
import UpdateTagModal from '../modal/tag/UpdateTagModal';
import { AnimatedButton, FakeButton } from '../primitive/Button';

type TagItemProps = {
	tag: Tag;
	onPress: () => void;
	tagStyle?: ClassInput;
	viewStyle?: 'grid' | 'list';
	rightActions?: () => void;
};

export const TagItem = ({
	tag,
	onPress,
	rightActions,
	tagStyle,
	viewStyle = 'grid'
}: TagItemProps) => {
	const modalRef = useRef<ModalRef>(null);

	const renderTagView = () => (
		<View
			style={twStyle(
				`h-auto flex-col justify-center gap-2.5 rounded-md border border-app-line/50 bg-app-box/50 p-2`,
				viewStyle === 'grid' ? 'w-[90px]' : 'w-full',
				tagStyle
			)}
		>
			<View style={tw`flex-row items-center justify-between`}>
				<View
					style={twStyle('h-[28px] w-[28px] rounded-full', {
						backgroundColor: tag.color!
					})}
				/>
				<Pressable onPress={() => modalRef.current?.present()}>
					<DotsThreeOutlineVertical
						weight="fill"
						size={20}
						color={tw.color('ink-faint')}
					/>
				</Pressable>
			</View>
			<Text style={tw`w-full max-w-[75px] text-xs font-bold text-white`} numberOfLines={1}>
				{tag.name}
			</Text>
		</View>
	);

	const renderRightActions = (
		progress: Animated.AnimatedInterpolation<number>,
		_dragX: Animated.AnimatedInterpolation<number>,
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
					tw`ml-5 flex flex-row items-center`,
					{ transform: [{ translateX: translate }] }
				]}
			>
				<UpdateTagModal tag={tag} ref={modalRef} onSubmit={() => swipeable.close()} />
				<AnimatedButton onPress={() => modalRef.current?.present()}>
					<Pen size={18} color="white" />
				</AnimatedButton>
				<DeleteTagModal
					tagId={tag.id}
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
		<Pressable onPress={onPress} testID="browse-tag">
			{viewStyle === 'grid' ? (
				renderTagView()
			) : (
				<Swipeable
					containerStyle={tw`rounded-md border border-app-line/50 bg-app-box/50 p-3`}
					enableTrackpadTwoFingerGesture
					renderRightActions={renderRightActions}
				>
					<View style={twStyle('h-auto flex-row items-center justify-between', tagStyle)}>
						<View style={tw`flex-1 flex-row items-center gap-2`}>
							<View
								style={twStyle('h-[28px] w-[28px] rounded-full', {
									backgroundColor: tag.color!
								})}
							/>
							<Text
								style={tw`w-full max-w-[75px] text-xs font-bold text-white`}
								numberOfLines={1}
							>
								{tag.name}
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
				</Swipeable>
			)}
			<TagModal ref={modalRef} tag={tag} />
		</Pressable>
	);
};

const BrowseTags = () => {
	const navigation = useNavigation<BrowseStackScreenProps<'Browse'>['navigation']>();

	const tags = useLibraryQuery(['tags.list']);

	useNodes(tags.data?.nodes);
	const tagData = useCache(tags.data?.items);

	const modalRef = useRef<ModalRef>(null);

	return (
		<View style={tw`gap-5`}>
			<View style={tw`w-full flex-row items-center justify-between px-7`}>
				<Text style={tw`text-lg font-bold text-white`}>Tags</Text>
				<View style={tw`flex-row gap-3`}>
					<Pressable onPress={() => navigation.navigate('Tags')}>
						<View
							style={tw`h-8 w-8 items-center justify-center rounded-md bg-accent ${
								tags.data?.nodes.length === 0 ? 'opacity-40' : 'opacity-100'
							}`}
						>
							<Eye weight="bold" size={18} style={tw`text-white`} />
						</View>
					</Pressable>
					<Pressable testID="add-tag" onPress={() => modalRef.current?.present()}>
						<View
							style={tw`h-8 w-8 items-center justify-center rounded-md border border-dashed border-ink-faint bg-transparent`}
						>
							<Plus weight="bold" size={18} style={tw`text-ink-faint`} />
						</View>
					</Pressable>
				</View>
			</View>
			<Fade color="mobile-screen" width={30} height="100%">
				<FlatList
					data={tagData}
					ListEmptyComponent={() => (
						<View
							style={tw`relative h-auto w-[85.5vw] flex-col items-center justify-center overflow-hidden rounded-md border border-dashed border-sidebar-line p-4`}
						>
							<Icon name="Tags" size={38} />
							<Text style={tw`mt-2 text-center font-medium text-ink-dull`}>
								You have no tags
							</Text>
						</View>
					)}
					renderItem={({ item }) => (
						<TagItem
							tag={item}
							onPress={() =>
								navigation.navigate('Tag', { id: item.id, color: item.color! })
							}
						/>
					)}
					keyExtractor={(item) => item.id.toString()}
					horizontal
					showsHorizontalScrollIndicator={false}
					contentContainerStyle={tw`px-7`}
					ItemSeparatorComponent={() => <View style={tw`w-2`} />}
				/>
			</Fade>
			<CreateTagModal ref={modalRef} />
		</View>
	);
};

export default BrowseTags;
