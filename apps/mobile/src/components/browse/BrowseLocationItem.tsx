import React from 'react';
import { Pressable, Text, View } from 'react-native';

import tw from '../../lib/tailwind';
import FolderIcon from '../icons/Folder';

interface BrowseLocationItemProps {
	folderName: string;
	onPress: () => void;
}

const BrowseLocationItem: React.FC<BrowseLocationItemProps> = (props) => {
	const { folderName, onPress } = props;
	return (
		<Pressable onPress={onPress}>
			<View style={tw.style('flex mb-[4px] flex-row items-center py-2 px-2 rounded')}>
				<FolderIcon size={18} />
				<Text style={tw.style('text-gray-300 text-sm font-medium ml-2')} numberOfLines={1}>
					{folderName}
				</Text>
			</View>
		</Pressable>
	);
};

export default BrowseLocationItem;
