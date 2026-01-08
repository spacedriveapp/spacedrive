import { SettingsRow, type SettingsRowProps } from "./SettingsRow";
import { Switch } from "./Switch";

interface SettingsToggleProps
  extends Omit<SettingsRowProps, "trailing" | "onPress"> {
  value: boolean;
  onValueChange: (value: boolean) => void;
}

export function SettingsToggle({
  value,
  onValueChange,
  ...props
}: SettingsToggleProps) {
  return (
    <SettingsRow
      {...props}
      trailing={<Switch onValueChange={onValueChange} value={value} />}
    />
  );
}
