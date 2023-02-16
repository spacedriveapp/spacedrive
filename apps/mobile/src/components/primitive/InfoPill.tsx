import React from 'react';
import { Text, TextStyle, View, ViewStyle } from 'react-native';
import { tw, twStyle } from '~/lib/tailwind';

type Props = {
	text: string;
	containerStyle?: ViewStyle;
	textStyle?: TextStyle;
};

export const InfoPill = (props: Props) => {
	return (
		<View
			style={twStyle(
				'shadow-app-shade/5 bg-app-highlight rounded-md border border-transparent px-[6px] py-[1px] shadow',
				props.containerStyle
			)}
		>
			<Text style={twStyle('text-ink-dull text-xs font-medium', props.textStyle)}>
				{props.text}
			</Text>
		</View>
	);
};

export function PlaceholderPill(props: Props) {
	return (
		<View
			style={twStyle(
				'shadow-app-shade/10 border-app-highlight rounded-md border border-dashed bg-transparent px-[6px] py-[1px] shadow',
				props.containerStyle
			)}
		>
			<Text style={twStyle('text-ink-faint/70 text-xs font-medium', props.textStyle)}>
				{props.text}
			</Text>
		</View>
	);
}
