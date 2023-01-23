import { VariantProps, cva } from 'class-variance-authority';
import { FC } from 'react';
import { TextInputProps as RNTextInputProps, TextInput } from 'react-native';
import tw from '~/lib/tailwind';

const input = cva(['text-sm leading-tight placeholder:rounded-md border shadow-sm'], {
	variants: {
		variant: {
			default: 'bg-app border-app-line text-ink'
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

export const Input: FC<InputProps> = ({ variant, ...props }) => {
	const { style, ...otherProps } = props;
	return (
		<TextInput
			placeholderTextColor={tw.color('ink-dull')}
			style={tw.style(input({ variant }), style as string)}
			{...otherProps}
		/>
	);
};
