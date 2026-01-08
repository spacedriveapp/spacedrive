import { View } from "react-native";
import { SettingsRow, type SettingsRowProps } from "./SettingsRow";

type SettingsLinkProps = Omit<SettingsRowProps, "trailing">;

export function SettingsLink(props: SettingsLinkProps) {
  return (
    <SettingsRow
      {...props}
      trailing={
        <View className="h-2 w-2 rotate-45 border-ink-dull border-t-2 border-r-2" />
      }
    />
  );
}
