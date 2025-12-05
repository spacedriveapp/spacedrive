import React from "react";
import { View, Text } from "react-native";
import Slider from "@react-native-community/slider";
import { SettingsRow, SettingsRowProps } from "./SettingsRow";

interface SettingsSliderProps extends Omit<SettingsRowProps, "trailing"> {
	value: number;
	minimumValue?: number;
	maximumValue?: number;
	onValueChange: (value: number) => void;
	showValue?: boolean;
}

export function SettingsSlider({
	value,
	minimumValue = 0,
	maximumValue = 100,
	onValueChange,
	showValue = true,
	...props
}: SettingsSliderProps) {
	return (
		<SettingsRow
			{...props}
			trailing={
				<View className="flex-row items-center gap-3 flex-1 ml-4">
					<Slider
						value={value}
						minimumValue={minimumValue}
						maximumValue={maximumValue}
						onValueChange={onValueChange}
						minimumTrackTintColor="hsl(220, 90%, 56%)"
						maximumTrackTintColor="hsl(235, 15%, 23%)"
						thumbTintColor="hsl(220, 90%, 56%)"
						style={{ flex: 1, height: 40 }}
					/>
					{showValue && (
						<Text className="text-ink-dull w-10 text-right">
							{Math.round(value)}
						</Text>
					)}
				</View>
			}
		/>
	);
}
