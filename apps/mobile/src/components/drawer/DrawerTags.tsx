import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { useNavigation } from '@react-navigation/native';
import { useRef } from 'react';
import { ColorValue, Pressable, Text, View } from 'react-native';
import { Tag, useCache, useLibraryQuery, useNodes } from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';
import { tw, twStyle } from '~/lib/tailwind';

import CollapsibleView from '../layout/CollapsibleView';
import CreateTagModal from '../modal/tag/CreateTagModal';
import { Button } from '../primitive/Button';

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
					'h-auto flex-row items-center gap-2 rounded border border-app-inputborder/50 bg-app-darkBox p-2'
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
	const navigation = useNavigation<DrawerNavigationHelpers>();

	useNodes(tags.data?.nodes);
	const tagData = useCache(tags.data?.items);

	const modalRef = useRef<ModalRef>(null);

	return (
		<CollapsibleView
			title="Tags"
			titleStyle={tw`text-sm font-semibold text-ink`}
			containerStyle={tw`mb-3 ml-1 mt-6`}
		>
			<View style={tw`mt-2 flex-row flex-wrap justify-between gap-1`}>
				<TagColumn tags={tagData} dataAmount={[0, 2]} />
				<TagColumn tags={tagData} dataAmount={[2, 4]} />
			</View>
			<View style={tw`mt-2 flex-row gap-2`}>
				{/* Add Tag */}
				<Button
					style={tw`flex-1 py-0`}
					onPress={() => modalRef.current?.present()}
					variant="dashed"
				>
					<Text style={tw`p-2 text-center text-xs font-medium text-ink-dull`}>+ Tag</Text>
				</Button>
				{/* See all tags */}
				{tagData?.length > 4 && (
					<Button
						onPress={() => {
							navigation.navigate('BrowseStack', {
								screen: 'Tags',
								initial: false
							});
						}}
						style={tw`flex-1 py-0`}
						variant="gray"
					>
						<Text style={tw`p-2 text-center text-xs font-medium text-ink`}>
							View all
						</Text>
					</Button>
				)}
			</View>
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
		<View style={tw`flex-1 flex-col gap-1`}>
			{tags?.slice(dataAmount[0], dataAmount[1]).map((tag: any) => (
				<DrawerTagItem
					key={tag.id}
					tagName={tag.name!}
					onPress={() =>
						navigation.navigate('BrowseStack', {
							screen: 'Tag',
							params: { id: tag.id, color: tag.color }
						})
					}
					tagColor={tag.color as ColorValue}
				/>
			))}
		</View>
	);
};

export default DrawerTags;
