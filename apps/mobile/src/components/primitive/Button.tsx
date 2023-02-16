import { VariantProps, cva } from 'class-variance-authority';
import { MotiPressable, MotiPressableProps } from 'moti/interactions';
import { FC, useMemo } from 'react';
import { Pressable, PressableProps } from 'react-native';
import { tw, twStyle } from '~/lib/tailwind';

const button = cva(['items-center justify-center rounded-md border shadow-sm'], {
	variants: {
		variant: {
			danger: ['border-red-800 bg-red-600'],
			gray: ['border-app-line bg-app-button'],
			dark_gray: ['border-app-box bg-app'],
			accent: ['border-accent-deep bg-accent shadow-app-shade/10 shadow-md']
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
		variant: 'gray',
		size: 'default'
	}
});

type ButtonProps = VariantProps<typeof button> & PressableProps;

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
