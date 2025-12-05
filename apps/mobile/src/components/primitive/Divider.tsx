import React from "react";
import { View, ViewProps } from "react-native";
import { cn } from "~/utils/cn";

interface DividerProps extends ViewProps {}

export function Divider({ className, ...props }: DividerProps) {
	return (
		<View
			className={cn("bg-app-divider my-2 h-px w-full", className)}
			{...props}
		/>
	);
}
