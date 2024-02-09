import { useNavigation } from '@react-navigation/native';
import { DotsThreeOutlineVertical, Eye, Plus } from 'phosphor-react-native';
import React, { useRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import { FlatList } from 'react-native-gesture-handler';
import { Tag, useCache, useLibraryQuery, useNodes } from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';
import { tw, twStyle } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';

import { Icon } from '../icons/Icon';
import Fade from '../layout/Fade';
import CreateTagModal from '../modal/tag/CreateTagModal';
import { TagModal } from '../modal/tag/TagModal';

type BrowseTagItemProps = {
	tag: Tag;
	onPress: () => void;
	tagStyle?: string;
};

export const BrowseTagItem: React.FC<BrowseTagItemProps> = ({ tag, onPress, tagStyle }) => {
	const modalRef = useRef<ModalRef>(null);
	return (
		<Pressable onPress={onPress} testID="browse-tag">
			<View
				style={twStyle(
					'h-auto w-[90px] flex-col justify-center gap-2.5 rounded-md border border-app-line/50 bg-app-box/50 p-2',
					tagStyle
				)}
			>
				<View style={tw`flex-row items-center justify-between`}>
					<View
						style={twStyle('h-[28px] w-[28px] rounded-full', {
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
				<Text
					style={tw`w-full max-w-[75px] text-xs font-bold text-white`}
					numberOfLines={1}
				>
					{tag.name}
				</Text>
			</View>
			<TagModal ref={modalRef} tag={tag} />
		</Pressable>
	);
};

const BrowseTags = () => {
	const navigation = useNavigation<BrowseStackScreenProps<'Browse'>['navigation']>();

	const tags = useLibraryQuery(['tags.list']);

	useNodes(tags.data?.nodes);
	const tagData = useCache(tags.data?.items);

	const modalRef = useRef<ModalRef>(null);

	return (
		<View style={tw`gap-5`}>
			<View style={tw`flex-row items-center justify-between w-full px-7`}>
				<Text style={tw`text-lg font-bold text-white`}>Tags</Text>
				<View style={tw`flex-row gap-3`}>
					<Pressable onPress={() => navigation.navigate('Tags')}>
						<View
							style={tw`h-8 w-8 items-center justify-center rounded-md bg-accent ${
								tags.data?.nodes.length === 0 ? 'opacity-40' : 'opacity-100'
							}`}
						>
							<Eye weight="bold" size={18} style={tw`text-white`} />
						</View>
					</Pressable>
					<Pressable testID="add-tag" onPress={() => modalRef.current?.present()}>
						<View
							style={tw`items-center justify-center w-8 h-8 bg-transparent border border-dashed rounded-md border-ink-faint`}
						>
							<Plus weight="bold" size={18} style={tw`text-ink-faint`} />
						</View>
					</Pressable>
				</View>
			</View>
			<Fade color="mobile-screen" width={30} height="100%">
				<FlatList
					data={tagData}
					ListEmptyComponent={() => (
						<View
							style={tw`relative h-auto w-[85.5vw] flex-col items-center justify-center overflow-hidden rounded-md border border-dashed border-sidebar-line p-4`}
						>
							<Icon name="Tags" size={38} />
							<Text style={tw`mt-2 font-medium text-center text-ink-dull`}>
								You have no tags
							</Text>
						</View>
					)}
					renderItem={({ item }) => (
						<BrowseTagItem
							tag={item}
							onPress={() =>
								navigation.navigate('Tag', { id: item.id, color: item.color! })
							}
						/>
					)}
					keyExtractor={(item) => item.id.toString()}
					horizontal
					showsHorizontalScrollIndicator={false}
					contentContainerStyle={tw`px-7`}
					ItemSeparatorComponent={() => <View style={tw`w-2`} />}
				/>
			</Fade>
			<CreateTagModal ref={modalRef} />
		</View>
	);
};

export default BrowseTags;
