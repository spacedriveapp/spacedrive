import React from 'react';
import { View, ViewProps } from 'react-native';
import { twStyle } from '~/lib/tailwind';

type CardProps = {
	children: React.ReactNode;
} & ViewProps;

const Card = ({ children, ...props }: CardProps) => {
	const { style, ...otherProps } = props;

	return (
		<View
			style={twStyle('border-app-line bg-app-overlay rounded-lg border px-4 py-3', style as string)}
			{...otherProps}
		>
			{children}
		</View>
	);
};

export default Card;
