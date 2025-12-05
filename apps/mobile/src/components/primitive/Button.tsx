import { cva, VariantProps } from "class-variance-authority";
import React, { FC } from "react";
import { Pressable, PressableProps, View, Text } from "react-native";
import { cn } from "~/utils/cn";

/**
 * Button component matching @packages/ui/src/Button.tsx variants
 * Adapted for React Native using Pressable instead of button/anchor
 */

const button = cva(
	"items-center justify-center rounded-xl border font-medium transition-opacity",
	{
		variants: {
			variant: {
				default: [
					"bg-transparent border-app-line/80",
					// Pressed state handled by Pressable render prop
				],
				subtle: [
					"border-transparent bg-transparent",
				],
				outline: [
					"border-sidebar-line/60 bg-transparent",
				],
				dotted: [
					"border-dashed border-sidebar-line/70 bg-transparent",
				],
				gray: [
					"bg-app-button border-app-line/80",
				],
				accent: [
					"bg-accent border-accent shadow-md",
				],
				colored: [
					"shadow-sm",
					// Custom background color should be passed via style prop
				],
				bare: "border-transparent bg-transparent",
			},
			size: {
				icon: "p-1",
				xs: "px-2 py-1",
				sm: "px-2.5 py-1.5",
				md: "px-3 py-2",
				lg: "px-4 py-2.5",
			},
			disabled: {
				true: "opacity-50",
			},
		},
		defaultVariants: {
			variant: "default",
			size: "sm",
		},
	},
);

const buttonText = cva("font-medium text-center", {
	variants: {
		variant: {
			default: "text-ink",
			subtle: "text-ink",
			outline: "text-ink",
			dotted: "text-ink-faint",
			gray: "text-ink",
			accent: "text-white",
			colored: "text-white",
			bare: "text-ink",
		},
		size: {
			icon: "text-sm",
			xs: "text-sm",
			sm: "text-sm",
			md: "text-base",
			lg: "text-lg",
		},
	},
	defaultVariants: {
		variant: "default",
		size: "sm",
	},
});

export type ButtonVariant = NonNullable<VariantProps<typeof button>["variant"]>;
export type ButtonSize = NonNullable<VariantProps<typeof button>["size"]>;

type ButtonProps = VariantProps<typeof button> &
	PressableProps & {
		children?: React.ReactNode;
	};

export const Button: FC<ButtonProps> = ({
	variant,
	size,
	disabled,
	children,
	className,
	...props
}) => {
	return (
		<Pressable
			disabled={disabled ?? false}
			className={cn(button({ variant, size, disabled }), className)}
			{...props}
		>
			{({ pressed }) => (
				<View className={cn(pressed && "opacity-70")}>
					{typeof children === "string" ? (
						<Text className={buttonText({ variant, size })}>
							{children}
						</Text>
					) : (
						children
					)}
				</View>
			)}
		</Pressable>
	);
};

// Fake button for layout purposes (no press handling)
type FakeButtonProps = VariantProps<typeof button> & {
	children?: React.ReactNode;
	className?: string;
};

export const FakeButton: FC<FakeButtonProps> = ({
	variant,
	size,
	children,
	className,
}) => {
	return (
		<View
			className={cn(
				button({ variant, size, disabled: false }),
				className,
			)}
		>
			{typeof children === "string" ? (
				<Text className={buttonText({ variant, size })}>
					{children}
				</Text>
			) : (
				children
			)}
		</View>
	);
};
