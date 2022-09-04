import { VariantProps, cva } from 'class-variance-authority';
import React from 'react';
import { TextInput as RNTextInput, TextInputProps as RNTextInputProps } from 'react-native';
import tw from '~/lib/tailwind';

const input = cva(['text-sm rounded-md border shadow-sm'], {
	variants: {
		variant: {
			default: 'bg-gray-550 border-gray-500 text-white'
		},
		size: {
			default: ['py-2', 'px-3']
		}
	},
	defaultVariants: {
		variant: 'default',
		size: 'default'
	}
});

type InputProps = VariantProps<typeof input> & RNTextInputProps;

export const TextInput: React.FC<InputProps> = ({ variant, ...props }) => {
	const { style, ...otherProps } = props;
	return (
		<RNTextInput
			placeholderTextColor={tw.color('gray-300')}
			style={tw.style(input({ variant }), style as string)}
			{...otherProps}
		/>
	);
};
