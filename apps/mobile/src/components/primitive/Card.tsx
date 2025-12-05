import React, { FC, ReactNode } from "react";
import { View, ViewProps } from "react-native";
import { cn } from "~/utils/cn";

interface CardProps extends ViewProps {
	children: ReactNode;
}

export const Card: FC<CardProps> = ({ children, className, ...props }) => {
	return (
		<View
			className={cn(
				"rounded-lg bg-app-card border border-app-divider p-4",
				className,
			)}
			{...props}
		>
			{children}
		</View>
	);
};

export const CardHeader: FC<CardProps> = ({
	children,
	className,
	...props
}) => {
	return (
		<View className={cn("mb-3", className)} {...props}>
			{children}
		</View>
	);
};

export const CardContent: FC<CardProps> = ({
	children,
	className,
	...props
}) => {
	return (
		<View className={cn("", className)} {...props}>
			{children}
		</View>
	);
};
