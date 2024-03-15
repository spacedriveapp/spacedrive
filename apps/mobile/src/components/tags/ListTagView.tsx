import { DotsThreeOutlineVertical } from 'phosphor-react-native';
import { Pressable, Text, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import { ClassInput } from 'twrnc';
import { Tag } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';

import { ModalRef } from '../layout/Modal';
import { TagModal } from '../modal/tag/TagModal';
import RightActions from './RightActions';

interface TagViewProps {
	tag: Tag;
	tagStyle?: ClassInput;
	modalRef: React.RefObject<ModalRef>;
}

const ListTagView = ({ tag, tagStyle, modalRef }: TagViewProps) => {
	return (
		<>
			<Swipeable
				containerStyle={tw`p-3 border rounded-md border-mobile-cardborder bg-mobile-card`}
				enableTrackpadTwoFingerGesture
				renderRightActions={(progress, _, swipeable) => (
					<>
						<RightActions progress={progress} swipeable={swipeable} tag={tag} />
					</>
				)}
			>
				<View style={twStyle('h-auto flex-row items-center justify-between', tagStyle)}>
					<View style={tw`flex-row items-center flex-1 gap-2`}>
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
							color={tw.color('ink-dull')}
						/>
					</Pressable>
				</View>
			</Swipeable>
			<TagModal ref={modalRef} tag={tag} />
		</>
	);
};

export default ListTagView;
