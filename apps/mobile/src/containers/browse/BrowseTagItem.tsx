import React from 'react';
import { ColorValue, Pressable, Text, View } from 'react-native';
import tw from '~/lib/tailwind';

type BrowseTagItemProps = {
	tagName: string;
	tagColor: ColorValue;
	onPress: () => void;
};

const BrowseTagItem: React.FC<BrowseTagItemProps> = (props) => {
	const { tagName, tagColor, onPress } = props;
	return (
		<Pressable onPress={onPress}>
			<View style={tw.style('flex mb-[4px] flex-row items-center py-2 px-2 rounded')}>
				<View style={tw.style('w-3 h-3 rounded-full', { backgroundColor: tagColor })} />
				<Text style={tw.style('text-gray-300 text-sm font-medium ml-2')} numberOfLines={1}>
					{tagName}
				</Text>
			</View>
		</Pressable>
	);
};

export default BrowseTagItem;
