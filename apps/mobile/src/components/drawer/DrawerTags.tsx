import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { useNavigation } from '@react-navigation/native';
import { useRef } from 'react';
import { ColorValue, Pressable, Text, View } from 'react-native';
import { useLibraryQuery } from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';
import tw, { twStyle } from '~/lib/tailwind';
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
		<Pressable onPress={onPress}>
			<View style={twStyle('mb-[4px] flex flex-row items-center rounded py-2 px-1')}>
				<View style={twStyle('h-3.5 w-3.5 rounded-full', { backgroundColor: tagColor })} />
				<Text style={twStyle('ml-2 text-sm font-medium text-gray-300')} numberOfLines={1}>
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

	const createTagModalRef = useRef<ModalRef>();

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
				<View style={tw`border-app-line/80 mt-1 rounded border border-dashed`}>
					<Text style={tw`p-2 text-center text-xs font-bold text-gray-400`}>Add Tag</Text>
				</View>
			</Pressable>
			<CreateTagModal ref={createTagModalRef} />
		</CollapsibleView>
	);
};

export default DrawerTags;
