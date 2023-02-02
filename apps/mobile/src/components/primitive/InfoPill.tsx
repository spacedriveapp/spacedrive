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
				'border px-1 border-transparent shadow shadow-app-shade/5 bg-app-highlight rounded-md',
				props.containerStyle
			)}
		>
			<Text style={tw.style('text-[11px] font-medium text-ink-dull', props.textStyle)}>
				{props.text}
			</Text>
		</View>
	);
};

export function PlaceholderPill(props: Props) {
	return (
		<View
			style={tw.style(
				'border shadow px-1 shadow-app-shade/10 rounded-md bg-transparent border-dashed border-app-highlight',
				props.containerStyle
			)}
		>
			<Text style={tw.style('text-[11px] font-medium text-ink-faint/70', props.textStyle)}>
				{props.text}
			</Text>
		</View>
	);
}
