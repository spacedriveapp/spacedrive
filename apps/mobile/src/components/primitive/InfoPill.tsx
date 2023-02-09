import React from 'react';
import { Text, TextStyle, View, ViewStyle } from 'react-native';
import tw from '~/lib/tailwind';

type Props = {
	text: string;
	containerStyle?: ViewStyle;
	textStyle?: TextStyle;
};

export const InfoPill = (props: Props) => {
	return (
		<View
			style={tw.style(
				'border px-[6px] py-[1px] border-transparent shadow shadow-app-shade/5 bg-app-highlight rounded-md',
				props.containerStyle
			)}
		>
			<Text style={tw.style('text-xs font-medium text-ink-dull', props.textStyle)}>
				{props.text}
			</Text>
		</View>
	);
};

export function PlaceholderPill(props: Props) {
	return (
		<View
			style={tw.style(
				'border shadow px-[6px] py-[1px] shadow-app-shade/10 rounded-md bg-transparent border-dashed border-app-highlight',
				props.containerStyle
			)}
		>
			<Text style={tw.style('text-xs font-medium text-ink-faint/70', props.textStyle)}>
				{props.text}
			</Text>
		</View>
	);
}
