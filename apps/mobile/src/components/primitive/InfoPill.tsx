import { IconProps } from 'phosphor-react-native';
import React, { ReactElement } from 'react';
import { Text, TextStyle, View, ViewStyle } from 'react-native';
import { twStyle } from '~/lib/tailwind';

type Props = {
	text: string;
	containerStyle?: ViewStyle;
	textStyle?: TextStyle;
	icon?: ReactElement<IconProps, any>;
};

export const InfoPill = (props: Props) => {
	return (
		<View
			style={twStyle(
				'rounded-md border border-transparent bg-app-highlight px-[6px] py-px',
				props.containerStyle
			)}
		>
			<Text style={twStyle('text-xs font-medium text-ink-dull', props.textStyle)}>
				{props.text}
			</Text>
		</View>
	);
};

export function PlaceholderPill(props: Props) {
	return (
		<View
			style={twStyle(
				'flex-row items-center gap-0.5 rounded-md border border-dashed border-app-lightborder bg-transparent px-[6px] py-px',
				props.containerStyle
			)}
		>
			{props.icon && props.icon}
			<Text style={twStyle('text-xs font-medium text-ink-faint', props.textStyle)}>
				{props.text}
			</Text>
		</View>
	);
}
