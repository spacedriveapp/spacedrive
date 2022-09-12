import { VariantProps, cva } from 'class-variance-authority';
import { MotiPressable, MotiPressableProps } from 'moti/interactions';
import React, { useMemo } from 'react';
import { Pressable, PressableProps } from 'react-native';
import tw from '~/lib/tailwind';

const button = cva(['border rounded-md items-center justify-center shadow-sm'], {
	variants: {
		variant: {
			default: 'bg-gray-50 border-gray-100',
			primary: ['bg-primary-600'],
			danger: ['bg-red-600'],
			gray: ['bg-gray-100 border-gray-200'],
			dark_gray: ['bg-gray-500 border-gray-600']
		},
		size: {
			default: ['py-1', 'px-3'],
			sm: ['py-1', 'px-2'],
			md: ['py-1.5', 'px-3'],
			lg: ['py-2', 'px-4']
		},
		disabled: {
			true: ['opacity-70']
		}
	},
	defaultVariants: {
		variant: 'default',
		size: 'default'
	}
});

type ButtonProps = VariantProps<typeof button> & PressableProps;

export const Button: React.FC<ButtonProps> = ({ variant, size, disabled, ...props }) => {
	const { style, ...otherProps } = props;
	return (
		<Pressable
			disabled={disabled}
			style={tw.style(button({ variant, size, disabled }), style as string)}
			{...otherProps}
		>
			{props.children}
		</Pressable>
	);
};

type AnimatedButtonProps = VariantProps<typeof button> & MotiPressableProps;

export const AnimatedButton: React.FC<AnimatedButtonProps> = ({
	variant,
	size,
	disabled,
	...props
}) => {
	const { style, containerStyle, ...otherProps } = props;
	return (
		<MotiPressable
			disabled={disabled}
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
			style={tw.style(button({ variant, size, disabled }), style as string)}
			// MotiPressable acts differently than Pressable so containerStyle might need to used to achieve the same effect
			containerStyle={containerStyle}
			{...otherProps}
		>
			{props.children}
		</MotiPressable>
	);
};
