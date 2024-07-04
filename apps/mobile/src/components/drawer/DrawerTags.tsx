import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { useNavigation } from '@react-navigation/native';
import { useRef } from 'react';
import { ColorValue, Pressable, Text, View } from 'react-native';
import { Tag, useLibraryQuery } from '@sd/client';
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
				style={tw`flex-row items-center gap-2 rounded-md border border-app-inputborder/50 bg-app-darkBox p-2`}
			>
				<View style={twStyle('h-4 w-4 rounded-full', { backgroundColor: tagColor })} />
				<Text style={twStyle('text-xs font-medium text-ink')} numberOfLines={1}>
					{tagName}
				</Text>
			</View>
		</Pressable>
	);
};

const DrawerTags = () => {
	const tags = useLibraryQuery(['tags.list']);
	const navigation = useNavigation<DrawerNavigationHelpers>();

	const tagData = tags.data || [];

	const modalRef = useRef<ModalRef>(null);

	return (
		<CollapsibleView
			title="Tags"
			titleStyle={tw`text-sm font-semibold text-ink`}
			containerStyle={tw`mb-3 mt-6`}
		>
			<View style={tw`mt-2 flex-row justify-between gap-1`}>
				<TagColumn tags={tagData} dataAmount={[0, 2]} />
				{tagData?.length > 2 && <TagColumn tags={tagData} dataAmount={[2, 4]} />}
			</View>
			<View style={tw`mt-2 flex-row flex-wrap gap-1`}>
				{/* Add Tag */}
				<Button
					style={twStyle(`py-0`, tagData?.length > 4 ? 'w-[49%]' : 'w-full')}
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
						style={tw`w-[49%] py-0`}
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
		<View
			style={twStyle(
				`gap-1`,
				tags && tags.length > 2 ? 'w-[49%] flex-col' : 'flex-1 flex-row'
			)}
		>
			{tags?.slice(dataAmount[0], dataAmount[1]).map((tag: Tag) => (
				<DrawerTagItem
					key={tag.id}
					tagName={tag.name!}
					onPress={() =>
						navigation.navigate('BrowseStack', {
							screen: 'Tag',
							params: { id: tag.id, color: tag.color },
							initial: false
						})
					}
					tagColor={tag.color as ColorValue}
				/>
			))}
		</View>
	);
};

export default DrawerTags;
