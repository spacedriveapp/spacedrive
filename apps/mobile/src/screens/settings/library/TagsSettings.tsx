import { Tag, useLibraryQuery } from '@sd/client';
import { CaretRight, Pen, Trash } from 'phosphor-react-native';
import { Animated, FlatList, Text, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import { AnimatedButton } from '~/components/primitive/Button';
import DeleteTagDialog from '~/containers/dialog/tag/DeleteTagDialog';
import UpdateTagDialog from '~/containers/dialog/tag/UpdateTagDialog';
import tw from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

function TagItem({ tag, index }: { tag: Tag; index: number }) {
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
				style={[tw`flex flex-row items-center`, { transform: [{ translateX: translate }] }]}
			>
				<UpdateTagDialog tag={tag} onSubmit={() => swipeable.close()}>
					<AnimatedButton size="md">
						<Pen size={18} color="white" />
					</AnimatedButton>
				</UpdateTagDialog>
				<DeleteTagDialog tagId={tag.id}>
					<AnimatedButton size="md" style={tw`mx-2`}>
						<Trash size={18} color="white" />
					</AnimatedButton>
				</DeleteTagDialog>
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
				<View style={tw`flex flex-row items-center justify-between`}>
					<View style={tw`flex flex-row`}>
						<View style={tw.style({ backgroundColor: tag.color }, 'w-4 h-4 rounded-full')} />
						<Text style={tw`ml-3 text-ink`}>{tag.name}</Text>
					</View>
					<CaretRight color={tw.color('ink-dull')} size={18} />
				</View>
			</View>
		</Swipeable>
	);
}

// TODO: Add "New Tag" button

const TagsSettingsScreen = ({ navigation }: SettingsStackScreenProps<'TagsSettings'>) => {
	const { data: tags } = useLibraryQuery(['tags.list']);

	return (
		<View style={tw`flex-1 px-3 py-4`}>
			<FlatList
				data={tags}
				keyExtractor={(item) => item.id.toString()}
				renderItem={({ item, index }) => <TagItem tag={item} index={index} />}
			/>
		</View>
	);
};

export default TagsSettingsScreen;
