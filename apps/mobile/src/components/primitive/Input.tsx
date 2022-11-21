import { VariantProps, cva } from 'class-variance-authority';
import { FC } from 'react';
import {
	TextInputProps as RNTextInputProps,
	Switch,
	SwitchProps,
	Text,
	TextInput,
	View
} from 'react-native';
import tw from '~/lib/tailwind';

const input = cva(['text-sm rounded-md border shadow-sm'], {
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

type SwitchInputProps = { title: string; description?: string } & SwitchProps;

export const SwitchInput: FC<SwitchInputProps> = ({ title, description, ...props }) => {
	return (
		<View style={tw`flex flex-row items-center justify-between pb-6`}>
			<View style={tw`w-[80%]`}>
				<Text style={tw`font-medium text-ink text-sm`}>{title}</Text>
				{description && <Text style={tw`text-ink-dull text-sm mt-2`}>{description}</Text>}
			</View>
			<Switch trackColor={{ false: tw.color('app-line'), true: tw.color('accent') }} {...props} />
		</View>
	);
};
