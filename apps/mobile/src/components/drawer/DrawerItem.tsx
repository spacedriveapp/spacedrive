import React from 'react';
import { Pressable, Text, View } from 'react-native';

import tw from '../../lib/tailwind';

type DrawerProps = {
	label: string;
	onPress: () => void;
	icon: JSX.Element;
	isSelected: boolean;
};

const DrawerItem: React.FC<DrawerProps> = (props) => {
	const { label, icon, onPress, isSelected } = props;
	return (
		<Pressable onPress={onPress} style={tw``}>
			<View
				style={tw.style(
					'flex mb-[4px] flex-row items-center py-2 px-2 rounded',
					isSelected && 'bg-primary'
				)}
			>
				{icon}
				<Text
					style={tw.style('text-gray-300 text-sm font-medium ml-2', isSelected && 'text-white')}
				>
					{label}
				</Text>
			</View>
		</Pressable>
	);
};

export default DrawerItem;
