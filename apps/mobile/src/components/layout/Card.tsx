import React from 'react';
import { View, ViewProps } from 'react-native';
import { ClassInput } from 'twrnc';
import { twStyle } from '~/lib/tailwind';

interface CardProps extends Omit<ViewProps, 'style'> {
	children: React.ReactNode;
	style?: ClassInput;
}

const Card = ({ children, style, ...props }: CardProps) => {
	return (
		<View
			{...props}
			style={twStyle('rounded-lg border border-app-cardborder bg-app-card p-2', style)}
		>
			{children}
		</View>
	);
};

export default Card;
