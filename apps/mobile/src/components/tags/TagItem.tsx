import { useRef } from 'react';
import { Pressable } from 'react-native';
import { ClassInput } from 'twrnc';
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
	style?: ClassInput;
};

export const TagItem = ({ tag, onPress, style, viewStyle = 'grid' }: TagItemProps) => {
	const modalRef = useRef<ModalRef>(null);
	return (
		<>
			<Pressable
				style={twStyle(viewStyle === 'grid' ? `m-1 w-[112px]` : `flex-1`, style)}
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
