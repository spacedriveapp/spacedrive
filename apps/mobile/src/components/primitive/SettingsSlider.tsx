import Slider from "@react-native-community/slider";
import { Text, View } from "react-native";
import { SettingsRow, type SettingsRowProps } from "./SettingsRow";

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
        <View className="ml-4 flex-1 flex-row items-center gap-3">
          <Slider
            maximumTrackTintColor="hsl(235, 15%, 23%)"
            maximumValue={maximumValue}
            minimumTrackTintColor="hsl(220, 90%, 56%)"
            minimumValue={minimumValue}
            onValueChange={onValueChange}
            style={{ flex: 1, height: 40 }}
            thumbTintColor="hsl(220, 90%, 56%)"
            value={value}
          />
          {showValue && (
            <Text className="w-10 text-right text-ink-dull">
              {Math.round(value)}
            </Text>
          )}
        </View>
      }
    />
  );
}
