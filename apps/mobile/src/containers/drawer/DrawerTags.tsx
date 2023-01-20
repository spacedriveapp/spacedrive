import { BottomSheetModal } from '@gorhom/bottom-sheet';
import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { useNavigation } from '@react-navigation/native';
import { useLibraryQuery } from '@sd/client';
import { useRef } from 'react';
import { ColorValue, Pressable, Text, View } from 'react-native';
import tw from '~/lib/tailwind';

import CollapsibleView from '../../components/layout/CollapsibleView';
import CreateTagModal from '../modal/tag/CreateTagModal';

type DrawerTagItemProps = {
	tagName: string;
	tagColor: ColorValue;
	onPress: () => void;
};

const DrawerTagItem: React.FC<DrawerTagItemProps> = (props) => {
	const { tagName, tagColor, onPress } = props;
	return (
		<Pressable onPress={onPress}>
			<View style={tw.style('flex mb-[4px] flex-row items-center py-2 px-1 rounded')}>
				<View style={tw.style('w-3.5 h-3.5 rounded-full', { backgroundColor: tagColor })} />
				<Text style={tw.style('text-gray-300 text-sm font-medium ml-2')} numberOfLines={1}>
					{tagName}
				</Text>
			</View>
		</Pressable>
	);
};

type DrawerTagsProp = {
	stackName: string;
};

const DrawerTags = ({ stackName }: DrawerTagsProp) => {
	const navigation = useNavigation<DrawerNavigationHelpers>();

	const { data: tags } = useLibraryQuery(['tags.list'], { keepPreviousData: true });

	const createTagModalRef = useRef<BottomSheetModal>();

	return (
		<CollapsibleView
			title="Tags"
			titleStyle={tw`text-sm font-semibold text-gray-300`}
			containerStyle={tw`mt-6 mb-3 ml-1`}
		>
			<View style={tw`mt-2`}>
				{tags?.map((tag) => (
					<DrawerTagItem
						key={tag.id}
						tagName={tag.name}
						onPress={() =>
							navigation.navigate(stackName, {
								screen: 'Tag',
								params: { id: tag.id }
							})
						}
						tagColor={tag.color as ColorValue}
					/>
				))}
			</View>
			{/* Add Tag */}
			<Pressable onPress={() => createTagModalRef.current.present()}>
				<View style={tw`border border-dashed rounded border-app-line border-opacity-80 mt-1`}>
					<Text style={tw`text-xs font-bold text-center text-gray-400 px-2 py-2`}>Add Tag</Text>
				</View>
			</Pressable>
			<CreateTagModal ref={createTagModalRef} />
		</CollapsibleView>
	);
};

export default DrawerTags;
