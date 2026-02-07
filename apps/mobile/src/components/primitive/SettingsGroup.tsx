import React, { Children, ReactElement, cloneElement } from "react";
import { View, Text } from "react-native";
import { cn } from "~/utils/cn";
import { SettingsRowProps } from "./SettingsRow";

interface SettingsGroupProps {
	header?: string;
	footer?: string;
	children: React.ReactNode;
	className?: string;
}

export function SettingsGroup({
	header,
	footer,
	children,
	className,
}: SettingsGroupProps) {
	const childArray = Children.toArray(children);
	const totalChildren = childArray.length;

	return (
		<View className={cn("mb-6", className)}>
			{/* Header */}
			{header && (
				<Text className="text-xs font-semibold text-ink-dull uppercase tracking-wider mb-2 px-4">
					{header}
				</Text>
			)}

			{/* Rows container */}
			<View className="rounded-[32px] overflow-hidden">
				{Children.map(children, (child, index) => {
					if (!React.isValidElement(child)) return child;

					return cloneElement(child as ReactElement<SettingsRowProps>, {
						isFirst: index === 0,
						isLast: index === totalChildren - 1,
					});
				})}
			</View>

			{/* Footer */}
			{footer && (
				<Text className="text-xs text-ink-faint mt-2 px-4">
					{footer}
				</Text>
			)}
		</View>
	);
}
