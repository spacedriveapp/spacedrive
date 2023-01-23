import React from 'react';
import { View, ViewProps } from 'react-native';
import tw from '~/lib/tailwind';

type CardProps = {
	children: React.ReactNode;
} & ViewProps;

const Card = ({ children, ...props }: CardProps) => {
	const { style, ...otherProps } = props;

	return (
		<View
			style={tw.style(
				'px-4 py-3 border border-app-line rounded-lg bg-app-overlay',
				style as string
			)}
			{...otherProps}
		>
			{children}
		</View>
	);
};

export default Card;
