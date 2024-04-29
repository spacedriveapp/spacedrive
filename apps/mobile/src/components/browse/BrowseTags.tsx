import { useNavigation } from '@react-navigation/native';
import { useCache, useLibraryQuery, useNodes } from '@sd/client';
import { DotsThreeOutline, Plus } from 'phosphor-react-native';
import React, { useRef } from 'react';
import { Text, View } from 'react-native';
import { ModalRef } from '~/components/layout/Modal';
import { tw } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';

import Empty from '../layout/Empty';
import CreateTagModal from '../modal/tag/CreateTagModal';
import { Button } from '../primitive/Button';
import { TagItem } from '../tags/TagItem';

const BrowseTags = () => {
	const navigation = useNavigation<BrowseStackScreenProps<'Browse'>['navigation']>();

	const tags = useLibraryQuery(['tags.list']);

	useNodes(tags.data?.nodes);
	const tagData = useCache(tags.data?.items);

	const modalRef = useRef<ModalRef>(null);

	return (
		<View style={tw`gap-5 px-6`}>
			<View style={tw`w-full flex-row items-center justify-between`}>
				<Text style={tw`text-lg font-bold text-white`}>Tags</Text>
				<View style={tw`flex-row gap-3`}>
					<Button
						style={tw`h-9 w-9 rounded-full`}
						variant="dashed"
						onPress={() => modalRef.current?.present()}
					>
						<Plus weight="bold" size={16} style={tw`text-ink`} />
					</Button>
					<Button
						testID="navigate-tags-screen"
						onPress={() => {
							navigation.navigate('Tags');
						}}
						style={tw`w-9 rounded-full`}
						variant="gray"
					>
						<DotsThreeOutline weight="fill" size={16} color={'white'} />
					</Button>
				</View>
			</View>
			<View style={tw`flex-row flex-wrap gap-2`}>
				{tagData?.length === 0 ? (
					<Empty description="You have not created any tags" icon="Tags" />
				) : (
					tagData
						?.slice(0, 3)
						.map((tag) => (
							<TagItem
								key={tag.id}
								tag={tag}
								onPress={() =>
									navigation.navigate('Tag', { id: tag.id, color: tag.color! })
								}
							/>
						))
				)}
			</View>
			<CreateTagModal ref={modalRef} />
		</View>
	);
};

export default BrowseTags;
