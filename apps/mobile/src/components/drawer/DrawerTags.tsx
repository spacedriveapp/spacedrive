import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { useNavigation } from '@react-navigation/native';
import { useRef } from 'react';
import { ColorValue, Pressable, Text, View } from 'react-native';
import { Tag, useCache, useLibraryQuery, useNodes } from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';
import { tw, twStyle } from '~/lib/tailwind';

import CollapsibleView from '../layout/CollapsibleView';
import CreateTagModal from '../modal/tag/CreateTagModal';

type DrawerTagItemProps = {
	tagName: string;
	tagColor: ColorValue;
	onPress: () => void;
};

const DrawerTagItem: React.FC<DrawerTagItemProps> = (props) => {
	const { tagName, tagColor, onPress } = props;
	return (
		<Pressable style={tw`flex-1`} onPress={onPress} testID="drawer-tag">
			<View
				style={twStyle(
					'bg-app-darkBox border border-app-inputborder/50 rounded-full h-auto flex-row items-center gap-2 rounded p-2'
				)}
			>
				<View style={twStyle('h-4 w-4 rounded-full', { backgroundColor: tagColor })} />
				<Text style={twStyle('text-xs font-medium text-gray-300')} numberOfLines={1}>
					{tagName}
				</Text>
			</View>
		</Pressable>
	);
};

const DrawerTags = () => {
	const tags = useLibraryQuery(['tags.list']);
	useNodes(tags.data?.nodes);
	const tagData = useCache(tags.data?.items);

	const modalRef = useRef<ModalRef>(null);

	return (
		<CollapsibleView
			title="Tags"
			titleStyle={tw`text-sm font-semibold text-ink`}
			containerStyle={tw`mt-6 mb-3 ml-1`}
		>
			<View style={tw`flex-row flex-wrap justify-between gap-1 mt-2`}>
				<TagColumn tags={tagData} dataAmount={[0, 2]} />
				<TagColumn tags={tagData} dataAmount={[2, 4]} />
			</View>
			{/* Add Tag */}
			<Pressable onPress={() => modalRef.current?.present()}>
				<View style={tw`mt-2 border border-dashed rounded border-app-line/80`}>
					<Text style={tw`p-2 text-xs font-bold text-center text-gray-400`}>Add Tag</Text>
				</View>
			</Pressable>
			<CreateTagModal ref={modalRef} />
		</CollapsibleView>
	);
};

interface TagColumnProps {
	tags?: Tag[];
	dataAmount: [start: number, end: number];
}

const TagColumn = ({ tags, dataAmount }: TagColumnProps) => {
	const navigation = useNavigation<DrawerNavigationHelpers>();
	return (
		<View style={tw`flex-col flex-1 gap-1`}>
			{tags?.slice(dataAmount[0], dataAmount[1]).map((tag: any) => (
				<DrawerTagItem
					key={tag.id}
					tagName={tag.name!}
					onPress={() =>
						navigation.navigate('BrowseStack', {
							screen: 'Tag',
							params: { id: tag.id }
						})
					}
					tagColor={tag.color as ColorValue}
				/>
			))}
		</View>
	);
};

export default DrawerTags;
