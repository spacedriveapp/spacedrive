import { DotsThreeOutlineVertical } from 'phosphor-react-native';
import { Pressable, Text, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import { ClassInput } from 'twrnc';
import { Tag } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';

import { ModalRef } from '../layout/Modal';
import RightActions from './RightActions';

interface ListTagProps {
	tag: Tag;
	tagStyle?: ClassInput;
	modalRef: React.RefObject<ModalRef>;
}

const ListTag = ({ tag, tagStyle, modalRef }: ListTagProps) => {
	return (
		<Swipeable
			containerStyle={tw`rounded-md border border-app-cardborder bg-app-card p-3`}
			enableTrackpadTwoFingerGesture
			renderRightActions={(progress, _, swipeable) => (
				<>
					<RightActions progress={progress} swipeable={swipeable} tag={tag} />
				</>
			)}
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
				<Pressable hitSlop={24} onPress={() => modalRef.current?.present()}>
					<DotsThreeOutlineVertical
						weight="fill"
						size={20}
						color={tw.color('ink-dull')}
					/>
				</Pressable>
			</View>
		</Swipeable>
	);
};

export default ListTag;
