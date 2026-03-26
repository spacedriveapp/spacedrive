import React from "react";
import { View } from "react-native";
import sharedColors from "@spaceui/tokens/raw-colors";

interface PageIndicatorProps {
	currentIndex: number;
	totalPages: number;
	activeColor?: string;
	inactiveColor?: string;
	/** Optional array of colors per page. If provided, overrides activeColor for that page. */
	pageColors?: (string | null)[];
}

export function PageIndicator({
	currentIndex,
	totalPages,
	activeColor = `hsl(${sharedColors.accent.DEFAULT})`,
	inactiveColor = `hsl(${sharedColors.app.line})`,
	pageColors,
}: PageIndicatorProps) {
	return (
		<View className="flex-row justify-center gap-2">
			{Array.from({ length: totalPages }).map((_, index) => {
				const isActive = currentIndex === index;
				const pageColor = pageColors?.[index];
				const backgroundColor = pageColor || (isActive ? activeColor : inactiveColor);

				return (
					<View
						key={index}
						className="h-2 rounded-full transition-all"
						style={{
							width: isActive ? 24 : 8,
							backgroundColor,
							opacity: isActive ? 1 : 0.3,
						}}
					/>
				);
			})}
		</View>
	);
}
