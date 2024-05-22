import React from 'react';
import { Text, View } from 'react-native';
import { ClassInput } from 'twrnc/dist/esm/types';
import { tw, twStyle } from '~/lib/tailwind';

interface Props {
	title: string;
	sub?: string;
	style?: ClassInput;
}

const SectionTitle = ({ title, sub, style }: Props) => {
	return (
		<View style={twStyle(style)}>
			<Text style={tw`leading-1 pb-1 text-lg font-bold text-white`}>{title}</Text>
			<Text style={tw`text-sm text-ink-dull`}>{sub}</Text>
		</View>
	);
};

export default SectionTitle;
