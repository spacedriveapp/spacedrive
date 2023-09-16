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
			style={twStyle(
				'rounded-lg border border-app-line bg-app-overlay px-4 py-5',
				style as string
			)}
			{...otherProps}
		>
			{children}
		</View>
	);
};

export default Card;
