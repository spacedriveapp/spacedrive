import { useNavigation } from '@react-navigation/native';
import { Eye, Plus } from 'phosphor-react-native';
import React, { useRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import { FlatList } from 'react-native-gesture-handler';
import { useCache, useLibraryQuery, useNodes } from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';
import { tw } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';

import Empty from '../layout/Empty';
import Fade from '../layout/Fade';
import CreateTagModal from '../modal/tag/CreateTagModal';
import { TagItem } from '../tags/TagItem';

const BrowseTags = () => {
	const navigation = useNavigation<BrowseStackScreenProps<'Browse'>['navigation']>();

	const tags = useLibraryQuery(['tags.list']);

	useNodes(tags.data?.nodes);
	const tagData = useCache(tags.data?.items);

	const modalRef = useRef<ModalRef>(null);

	return (
		<View style={tw`gap-3`}>
			<View style={tw`w-full flex-row items-center justify-between px-6`}>
				<Text style={tw`text-lg font-bold text-white`}>Tags</Text>
				<View style={tw`flex-row gap-3`}>
					<Pressable
						testID="navigate-tags-screen"
						onPress={() => navigation.navigate('Tags')}
					>
						<View style={tw`h-8 w-8 items-center justify-center rounded-md bg-accent`}>
							<Eye weight="bold" size={18} style={tw`text-white`} />
						</View>
					</Pressable>
					<Pressable onPress={() => modalRef.current?.present()}>
						<View
							style={tw`h-8 w-8 items-center justify-center rounded-md border border-dashed border-app-iconborder bg-transparent`}
						>
							<Plus weight="bold" size={18} style={tw`text-ink`} />
						</View>
					</Pressable>
				</View>
			</View>
			<Fade color="black" width={30} height="100%">
				<FlatList
					data={tagData}
					ListEmptyComponent={
						<Empty description="You have not created any tags" icon="Tags" />
					}
					renderItem={({ item }) => (
						<TagItem
							tag={item}
							onPress={() =>
								navigation.navigate('Tag', { id: item.id, color: item.color! })
							}
						/>
					)}
					keyExtractor={(item) => item.id.toString()}
					horizontal
					showsHorizontalScrollIndicator={false}
					contentContainerStyle={tw`w-full px-6`}
					ItemSeparatorComponent={() => <View style={tw`w-2`} />}
				/>
			</Fade>
			<CreateTagModal ref={modalRef} />
		</View>
	);
};

export default BrowseTags;
