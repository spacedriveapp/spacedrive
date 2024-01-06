import { useNavigation } from '@react-navigation/native';
import { DotsThreeOutlineVertical, Eye, Plus } from 'phosphor-react-native';
import React, { useRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import { FlatList } from 'react-native-gesture-handler';
import { Tag, useCache, useLibraryQuery, useNodes } from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';
import { tw, twStyle } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';

import Fade from '../layout/Fade';
import CreateTagModal from '../modal/tag/CreateTagModal';
import { TagModal } from '../modal/tag/TagModal';

type BrowseTagItemProps = {
	tag: Tag;
	onPress: () => void;
};

const BrowseTagItem: React.FC<BrowseTagItemProps> = ({ tag, onPress }) => {
	const modalRef = useRef<ModalRef>(null);
	return (
		<Pressable onPress={onPress} testID="browse-tag">
			<View
				style={tw`h-fit w-[90px] flex-col justify-center gap-2.5 rounded-md border border-sidebar-line/50 bg-sidebar-box p-2`}
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
				<Text style={tw`text-xl font-bold text-white`}>Tags</Text>
				<View style={tw`flex-row gap-3`}>
					<Pressable>
						<View style={tw`items-center justify-center w-8 h-8 rounded-md bg-accent`}>
							<Eye weight="bold" size={18} style={tw`text-white`} />
						</View>
					</Pressable>
					<Pressable onPress={() => modalRef.current?.present()}>
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
					renderItem={({ item }) => (
						<BrowseTagItem
							tag={item}
							onPress={() => navigation.navigate('Tag', { id: item.id })}
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
