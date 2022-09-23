import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { useNavigation } from '@react-navigation/native';
import React from 'react';
import { ColorValue, Pressable, Text, View } from 'react-native';
import tw from '~/lib/tailwind';

import CollapsibleView from '../../components/layout/CollapsibleView';

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
				<View style={tw.style('w-3 h-3 rounded-full', { backgroundColor: tagColor })} />
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

const placeholderTagsData = [
	{
		id: 1,
		name: 'Funny',
		color: tw.color('blue-500')
	},
	{
		id: 2,
		name: 'Twitch',
		color: tw.color('purple-500')
	},
	{
		id: 3,
		name: 'BlackMagic',
		color: tw.color('red-500')
	}
];

const DrawerTags = ({ stackName }: DrawerTagsProp) => {
	const navigation = useNavigation<DrawerNavigationHelpers>();

	return (
		<CollapsibleView
			title="Tags"
			titleStyle={tw`text-sm font-semibold text-gray-300`}
			containerStyle={tw`mt-6 mb-3`}
		>
			<View style={tw`mt-2`}>
				{placeholderTagsData.map((tag) => (
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
		</CollapsibleView>
	);
};

export default DrawerTags;
