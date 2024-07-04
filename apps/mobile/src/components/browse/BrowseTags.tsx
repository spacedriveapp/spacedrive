import { useNavigation } from '@react-navigation/native';
import { Plus } from 'phosphor-react-native';
import React, { useRef, useState } from 'react';
import { FlatList, Text, View } from 'react-native';
import { useLibraryQuery } from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';
import { tw, twStyle } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';

import Empty from '../layout/Empty';
import Fade from '../layout/Fade';
import CreateTagModal from '../modal/tag/CreateTagModal';
import { Button } from '../primitive/Button';
import { TagItem } from '../tags/TagItem';

const BrowseTags = () => {
	const navigation = useNavigation<BrowseStackScreenProps<'Browse'>['navigation']>();

	const tags = useLibraryQuery(['tags.list']);
	const tagData = tags.data;

	const modalRef = useRef<ModalRef>(null);
	const [showAll, setShowAll] = useState(false);

	return (
		<View style={tw`gap-5`}>
			<View style={tw`w-full flex-row items-center justify-between px-5`}>
				<Text style={tw`text-lg font-bold text-white`}>Tags</Text>
				<View style={tw`flex-row gap-3`}>
					<Button
						testID="show-all-tags-button"
						style={twStyle(`rounded-full`, {
							borderColor: showAll
								? tw.color('accent')
								: tw.color('border-app-lightborder')
						})}
						variant="outline"
						onPress={() => setShowAll((prev) => !prev)}
					>
						<Text style={tw`text-xs text-ink`}>
							{showAll ? 'Show less' : 'Show all'} ({tagData?.length})
						</Text>
					</Button>
					<Button
						testID="create-tag-button"
						onPress={() => modalRef.current?.present()}
						style={tw`flex-row gap-1 rounded-full`}
						variant="gray"
					>
						<Plus size={10} weight="bold" style={tw`text-white`} />
						<Text style={tw`text-xs text-ink`}>Add</Text>
					</Button>
				</View>
			</View>
			<View style={tw`relative -m-1`}>
				<Fade color="black" width={20} height="100%">
					<FlatList
						data={tagData}
						ListEmptyComponent={
							<Empty description="You have not created any tags" icon="Tags" />
						}
						numColumns={showAll ? 3 : 1}
						contentContainerStyle={twStyle(tagData?.length === 0 && 'w-full', 'px-5')}
						horizontal={showAll ? false : true}
						key={showAll ? '_tags' : 'alltagcols'}
						keyExtractor={(item) => item.id.toString()}
						scrollEnabled={showAll ? false : true}
						showsHorizontalScrollIndicator={false}
						renderItem={({ item }) => (
							<TagItem
								style={twStyle(showAll && 'max-w-[31%] flex-1')}
								key={item.id}
								tag={item}
								onPress={() =>
									navigation.navigate('Tag', { id: item.id, color: item.color! })
								}
							/>
						)}
					/>
				</Fade>
			</View>
			<CreateTagModal ref={modalRef} />
		</View>
	);
};

export default BrowseTags;
