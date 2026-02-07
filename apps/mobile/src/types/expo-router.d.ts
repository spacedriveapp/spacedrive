// Type augmentation for expo-router unstable native tabs
// The `name` prop exists at runtime but is missing from types

declare module "expo-router/unstable-native-tabs" {
	import type { ComponentProps } from "react";

	export interface IconProps {
		sf?: string;
		name?: string;
	}

	export const Icon: React.FC<IconProps>;
	export const Label: React.FC<{ children: React.ReactNode }>;

	export interface NativeTabsProps {
		backgroundColor?: string | null;
		disableTransparentOnScrollEdge?: boolean;
		iconColor?: string;
		labelStyle?: object;
		children?: React.ReactNode;
	}

	export const NativeTabs: React.FC<NativeTabsProps> & {
		Trigger: React.FC<{ name: string; children: React.ReactNode }>;
	};
}
