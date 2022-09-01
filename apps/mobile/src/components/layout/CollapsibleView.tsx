import tw from '@app/lib/tailwind';
import { Ionicons } from '@expo/vector-icons';
import { MotiView } from 'moti';
import React, { useReducer } from 'react';
import { Pressable, StyleProp, Text, TextStyle, View, ViewStyle } from 'react-native';
import { ChevronRightIcon } from 'react-native-heroicons/solid';

import { AnimatedHeight } from '../animation/layout';

type CollapsibleViewProps = {
	title: string;
	titleStyle?: StyleProp<TextStyle>;
	children: React.ReactNode;
	containerStyle?: StyleProp<ViewStyle>;
};

const CollapsibleView = ({ title, titleStyle, containerStyle, children }: CollapsibleViewProps) => {
	const [hide, toggle] = useReducer((hide) => !hide, false);

	return (
		<View style={containerStyle}>
			<Pressable onPress={toggle} style={tw`flex flex-row justify-between items-center`}>
				<Text style={titleStyle} selectable={false}>
					{title}
				</Text>
				<MotiView
					animate={{
						rotateZ: hide ? '0deg' : '90deg',
						translateX: hide ? 0 : 5,
						translateY: hide ? 0 : 5
					}}
					transition={{ type: 'spring' }}
				>
					<ChevronRightIcon size={18} style={tw`text-gray-200 mr-3`} />
				</MotiView>
			</Pressable>
			<AnimatedHeight hide={hide}>{children}</AnimatedHeight>
		</View>
	);
};

export default CollapsibleView;
