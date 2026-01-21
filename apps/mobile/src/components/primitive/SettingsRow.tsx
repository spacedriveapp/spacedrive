import React, { ReactNode } from "react";
import { View, Text, Pressable, PressableProps } from "react-native";
import { cn } from "~/utils/cn";

export interface SettingsRowProps extends Omit<PressableProps, "children"> {
	icon?: ReactNode;
	label: string;
	description?: string;
	trailing?: ReactNode;
	isFirst?: boolean;
	isLast?: boolean;
	onPress?: () => void;
}

export function SettingsRow({
	icon,
	label,
	description,
	trailing,
	isFirst,
	isLast,
	onPress,
	className,
	...props
}: SettingsRowProps) {
	const Component = onPress ? Pressable : View;

	return (
		<>
			<Component
				onPress={onPress}
				className={cn(
					"flex-row items-center px-6 py-3 bg-app-box min-h-[56px]",
					isFirst && "rounded-t-[32px]",
					isLast && "rounded-b-[32px]",
					onPress && "active:bg-app-selected",
					className
				)}
				{...props}
			>
				{/* Icon */}
				{icon && <View className="mr-3">{icon}</View>}

				{/* Label & Description */}
				<View className="flex-1">
					<Text className="text-lg text-ink">{label}</Text>
					{description && (
						<Text className="text-sm text-ink-dull mt-0.5">
							{description}
						</Text>
					)}
				</View>

				{/* Trailing accessory */}
				{trailing && <View className="ml-3">{trailing}</View>}
			</Component>

			{/* Divider (not after last item) */}
			{!isLast && (
				<View className="bg-app-box">
					<View
						className="h-px bg-app-line"
						style={{ marginLeft: icon ? 60 : 24 }}
					/>
				</View>
			)}
		</>
	);
}
