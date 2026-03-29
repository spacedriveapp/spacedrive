import React from "react";
import { View } from "react-native";

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
	activeColor = "hsl(208, 100%, 57%)",
	inactiveColor = "hsl(235, 15%, 23%)",
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
