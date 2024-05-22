import { useRef } from 'react';
import { Pressable } from 'react-native';
import { Tag } from '@sd/client';
import { twStyle } from '~/lib/tailwind';

import { ModalRef } from '../layout/Modal';
import { TagModal } from '../modal/tag/TagModal';
import GridTag from './GridTag';
import ListTag from './ListTag';

type TagItemProps = {
	tag: Tag;
	onPress: () => void;
	viewStyle?: 'grid' | 'list';
};

export const TagItem = ({ tag, onPress, viewStyle = 'grid' }: TagItemProps) => {
	const modalRef = useRef<ModalRef>(null);
	return (
		<>
			<Pressable
				style={twStyle(viewStyle === 'grid' ? `w-[31.5%]` : `flex-1`)}
				onPress={onPress}
				testID="browse-tag"
			>
				{viewStyle === 'grid' ? (
					<>
						<GridTag tag={tag} modalRef={modalRef} />
						<TagModal ref={modalRef} tag={tag} />
					</>
				) : (
					<ListTag tag={tag} />
				)}
			</Pressable>
		</>
	);
};
