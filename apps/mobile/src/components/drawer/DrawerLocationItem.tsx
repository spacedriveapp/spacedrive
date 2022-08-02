import React from 'react';
import { Pressable, Text, View } from 'react-native';

import tw from '../../lib/tailwind';
import FolderIcon from '../icons/Folder';

interface DrawerLocationItemProps {
	folderName: string;
	onPress: () => void;
	isSelected: boolean;
}

const DrawerLocationItem: React.FC<DrawerLocationItemProps> = (props) => {
	const { folderName, onPress, isSelected } = props;
	return (
		<Pressable onPress={onPress} style={tw``}>
			<View
				style={tw.style(
					'flex mb-[4px] flex-row items-center py-2 px-2 rounded',
					isSelected && 'bg-primary'
				)}
			>
				<FolderIcon size={18} isWhite={isSelected} />
				<Text
					style={tw.style('text-gray-300 text-sm font-medium ml-2', isSelected && 'text-white')}
					numberOfLines={1}
				>
					{folderName}
				</Text>
			</View>
		</Pressable>
	);
};

export default DrawerLocationItem;
