import React, { FC, ReactNode } from "react";
import { View, ScrollView, ScrollViewProps, ViewProps } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { cn } from "~/utils/cn";

interface ScreenContainerProps extends ViewProps {
	children: ReactNode;
	scrollable?: boolean;
	scrollViewProps?: ScrollViewProps;
}

export const ScreenContainer: FC<ScreenContainerProps> = ({
	children,
	scrollable = false,
	scrollViewProps,
	className,
	...props
}) => {
	const insets = useSafeAreaInsets();

	if (scrollable) {
		return (
			<ScrollView
				className={cn("flex-1 bg-app", className)}
				contentContainerStyle={{
					paddingTop: insets.top,
					paddingBottom: insets.bottom + 16,
					paddingHorizontal: 16,
				}}
				showsVerticalScrollIndicator={false}
				{...scrollViewProps}
			>
				{children}
			</ScrollView>
		);
	}

	return (
		<View
			className={cn("flex-1 bg-app px-4", className)}
			style={{
				paddingTop: insets.top,
				paddingBottom: insets.bottom,
			}}
			{...props}
		>
			{children}
		</View>
	);
};
