import type { FC } from "react";
import { Switch as RNSwitch, type SwitchProps, Text, View } from "react-native";

export const Switch: FC<SwitchProps> = (props) => {
  return (
    <RNSwitch
      ios_backgroundColor="hsl(235, 10%, 16%)"
      thumbColor="#fff"
      trackColor={{
        false: "hsl(235, 10%, 16%)",
        true: "hsl(208, 100%, 57%)",
      }}
      {...props}
    />
  );
};

interface SwitchContainerProps extends SwitchProps {
  title: string;
  description?: string;
}

export const SwitchContainer: FC<SwitchContainerProps> = ({
  title,
  description,
  ...props
}) => {
  return (
    <View className="flex-row items-center justify-between py-3">
      <View className="flex-1 pr-4">
        <Text className="font-medium text-ink text-sm">{title}</Text>
        {description && (
          <Text className="mt-1 text-ink-dull text-sm">{description}</Text>
        )}
      </View>
      <Switch {...props} />
    </View>
  );
};
