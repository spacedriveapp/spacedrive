import { useRef } from 'react';
import { Pressable } from 'react-native';
import { ClassInput } from 'twrnc';
import { Tag } from '@sd/client';
import { tw } from '~/lib/tailwind';

import { ModalRef } from '../layout/Modal';
import { TagModal } from '../modal/tag/TagModal';
import GridTag from './GridTag';
import ListTag from './ListTag';

type TagItemProps = {
	tag: Tag;
	onPress: () => void;
	tagStyle?: ClassInput;
	viewStyle?: 'grid' | 'list';
};

export const TagItem = ({ tag, onPress, tagStyle, viewStyle = 'grid' }: TagItemProps) => {
	const modalRef = useRef<ModalRef>(null);
	return (
		<Pressable style={tw`flex-1`} onPress={onPress} testID="browse-tag">
			{viewStyle === 'grid' ? (
				<GridTag tag={tag} tagStyle={tagStyle} modalRef={modalRef} />
			) : (
				<ListTag tag={tag} modalRef={modalRef} />
			)}
			<TagModal ref={modalRef} tag={tag} />
		</Pressable>
	);
};
