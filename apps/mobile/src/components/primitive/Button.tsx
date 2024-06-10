import { cva, VariantProps } from 'class-variance-authority';
import { MotiPressable, MotiPressableProps } from 'moti/interactions';
import { FC, useMemo } from 'react';
import { Pressable, PressableProps, View, ViewProps } from 'react-native';
import { twStyle } from '~/lib/tailwind';

const button = cva(['items-center justify-center rounded-md border shadow-sm'], {
	variants: {
		variant: {
			danger: ['border-red-800 bg-red-600 shadow-none'],
			gray: ['border-app-box bg-app shadow-none'],
			darkgray: ['border-app-box bg-app shadow-none'],
			accent: ['border-accent-deep bg-accent shadow-md shadow-app-shade/10'],
			outline: ['border border-app-inputborder bg-transparent shadow-none'],
			transparent: ['border-0 bg-transparent shadow-none'],
			dashed: ['border border-dashed border-app-line bg-transparent shadow-none']
		},
		size: {
			default: ['py-1.5', 'px-3'],
			sm: ['py-1', 'px-2'],
			lg: ['py-2', 'px-4']
		},
		disabled: {
			true: ['opacity-70']
		}
	},
	defaultVariants: {
		variant: 'gray',
		size: 'default'
	}
});

type ButtonProps = VariantProps<typeof button> & PressableProps;
export type ButtonVariants = ButtonProps['variant'];

export const Button: FC<ButtonProps> = ({ variant, size, disabled, ...props }) => {
	const { style, ...otherProps } = props;
	return (
		<Pressable
			disabled={disabled}
			style={twStyle(button({ variant, size, disabled }), style as string)}
			{...otherProps}
		>
			{props.children}
		</Pressable>
	);
};

type AnimatedButtonProps = VariantProps<typeof button> & MotiPressableProps;

export const AnimatedButton: FC<AnimatedButtonProps> = ({ variant, size, disabled, ...props }) => {
	const { style, containerStyle, ...otherProps } = props;
	return (
		<MotiPressable
			disabled={disabled}
			// @ts-ignore
			animate={useMemo(
				() =>
					({ hovered, pressed }) => {
						'worklet';
						return {
							opacity: hovered || pressed ? 0.7 : 1,
							scale: hovered || pressed ? 0.97 : 1
						};
					},
				[]
			)}
			style={twStyle(button({ variant, size, disabled }), style as string)}
			// MotiPressable acts differently than Pressable so containerStyle might need to used to achieve the same effect
			containerStyle={containerStyle}
			{...otherProps}
		>
			{props.children}
		</MotiPressable>
	);
};

// Useful for when you want to replicate a button but don't want to deal with the pressable logic (e.g. you need to disable the inner pressable)
type FakeButtonProps = VariantProps<typeof button> & ViewProps;

export const FakeButton: FC<FakeButtonProps> = ({ variant, size, ...props }) => {
	const { style, ...otherProps } = props;
	return (
		<View
			style={twStyle(button({ variant, size, disabled: false }), style as string)}
			{...otherProps}
		>
			{props.children}
		</View>
	);
};
