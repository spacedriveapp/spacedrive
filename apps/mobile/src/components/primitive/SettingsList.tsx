import React, { ReactNode } from "react";
import { ScrollView, ScrollViewProps } from "react-native";
import { cn } from "~/utils/cn";

interface SettingsListProps extends ScrollViewProps {
	children: ReactNode;
}

export function SettingsList({
	children,
	className,
	contentContainerStyle,
	...props
}: SettingsListProps) {
	return (
		<ScrollView
			className={cn("flex-1", className)}
			contentContainerStyle={[
				{ paddingHorizontal: 16, paddingVertical: 16 },
				contentContainerStyle,
			]}
			{...props}
		>
			{children}
		</ScrollView>
	);
}
