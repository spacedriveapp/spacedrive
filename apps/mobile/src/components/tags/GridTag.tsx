import { DotsThreeOutlineVertical } from 'phosphor-react-native';
import { Pressable, Text, View } from 'react-native';
import { Tag } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';

import Card from '../layout/Card';
import { ModalRef } from '../layout/Modal';

interface GridTagProps {
	tag: Tag;
	modalRef: React.RefObject<ModalRef>;
}

const GridTag = ({ tag, modalRef }: GridTagProps) => {
	return (
		<Card style={twStyle(`h-auto w-full flex-col justify-center gap-3`)}>
			<View style={tw`flex-row items-center justify-between`}>
				<View
					style={twStyle('h-5 w-5 rounded-full', {
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
		</Card>
	);
};

export default GridTag;
