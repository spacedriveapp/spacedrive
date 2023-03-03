import { VariantProps, cva } from 'class-variance-authority';
import { FC } from 'react';
import { TextInputProps as RNTextInputProps, TextInput } from 'react-native';
import { tw, twStyle } from '~/lib/tailwind';

const input = cva(['rounded-md border text-sm leading-tight shadow-sm'], {
	variants: {
		variant: {
			default: 'border-app-line bg-app text-ink'
		},
		size: {
			default: ['py-2', 'px-3'],
			md: ['py-2.5', 'px-3.5']
		}
	},
	defaultVariants: {
		variant: 'default',
		size: 'default'
	}
});

type InputProps = VariantProps<typeof input> & RNTextInputProps;

export const Input: FC<InputProps> = ({ variant, size, ...props }) => {
	const { style, ...otherProps } = props;
	return (
		<TextInput
			placeholderTextColor={tw.color('ink-dull')}
			style={twStyle(input({ variant, size }), style as string)}
			{...otherProps}
		/>
	);
};
