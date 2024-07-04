import { DotsThreeVertical } from 'phosphor-react-native';
import { useRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import { ClassInput } from 'twrnc';
import { Tag } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';

import RightActions from './RightActions';

interface ListTagProps {
	tag: Tag;
	tagStyle?: ClassInput;
}

const ListTag = ({ tag, tagStyle }: ListTagProps) => {
	const swipeRef = useRef<Swipeable>(null);

	return (
		<Swipeable
			ref={swipeRef}
			containerStyle={tw`h-12 flex-col justify-center rounded-md border border-app-cardborder bg-app-card`}
			enableTrackpadTwoFingerGesture
			renderRightActions={(progress, _, swipeable) => (
				<RightActions progress={progress} swipeable={swipeable} tag={tag} />
			)}
		>
			<View style={twStyle('flex-row items-center justify-between px-3', tagStyle)}>
				<View style={tw`flex-1 flex-row items-center gap-2`}>
					<View
						style={twStyle('h-5 w-5 rounded-full', {
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
				<Pressable onPress={() => swipeRef.current?.openRight()}>
					<DotsThreeVertical weight="bold" size={20} color={tw.color('ink-dull')} />
				</Pressable>
			</View>
		</Swipeable>
	);
};

export default ListTag;
