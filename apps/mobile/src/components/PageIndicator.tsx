import React from "react";
import { View } from "react-native";
import Animated, {
	useAnimatedStyle,
	withTiming,
	Easing,
} from "react-native-reanimated";
import sharedColors from "@sd/ui/style/colors";

const timingConfig = {
	duration: 200,
	easing: Easing.out(Easing.cubic),
};

interface PageIndicatorProps {
	currentIndex: number;
	totalPages: number;
	activeColor?: string;
	inactiveColor?: string;
	/** Optional array of colors per page. If provided, overrides activeColor for that page. */
	pageColors?: (string | null)[];
}

function IndicatorDot({
	isActive,
	color,
	inactiveColor,
}: {
	isActive: boolean;
	color: string;
	inactiveColor: string;
}) {
	const animatedStyle = useAnimatedStyle(() => ({
		width: withTiming(isActive ? 24 : 8, timingConfig),
		opacity: withTiming(isActive ? 1 : 0.3, timingConfig),
		backgroundColor: withTiming(isActive ? color : inactiveColor, timingConfig),
	}));

	return (
		<Animated.View
			style={[
				{
					height: 8,
					borderRadius: 4,
				},
				animatedStyle,
			]}
		/>
	);
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
				const pageColor = pageColors?.[index] || activeColor;

				return (
					<IndicatorDot
						key={index}
						isActive={isActive}
						color={pageColor}
						inactiveColor={inactiveColor}
					/>
				);
			})}
		</View>
	);
}
