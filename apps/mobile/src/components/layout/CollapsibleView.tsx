import { MotiView } from 'moti';
import { CaretRight } from 'phosphor-react-native';
import { PropsWithChildren, useReducer } from 'react';
import { Pressable, StyleProp, Text, TextStyle, View, ViewStyle } from 'react-native';
import tw from '~/lib/tailwind';

import { AnimatedHeight } from '../animation/layout';

type CollapsibleViewProps = PropsWithChildren<{
	title: string;
	titleStyle?: StyleProp<TextStyle>;
	containerStyle?: StyleProp<ViewStyle>;
}>;

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
					transition={{ type: 'timing', duration: 150 }}
				>
					<CaretRight color="white" weight="bold" size={16} style={tw`mr-3`} />
				</MotiView>
			</Pressable>
			<AnimatedHeight hide={hide}>{children}</AnimatedHeight>
		</View>
	);
};

export default CollapsibleView;
