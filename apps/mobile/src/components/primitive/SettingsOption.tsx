import { Text } from "react-native";
import { SettingsRow, type SettingsRowProps } from "./SettingsRow";

interface SettingsOptionProps extends SettingsRowProps {
  value?: string;
}

export function SettingsOption({ value, ...props }: SettingsOptionProps) {
  return (
    <SettingsRow
      {...props}
      trailing={
        value ? <Text className="mr-2 text-ink-dull">{value}</Text> : undefined
      }
    />
  );
}
