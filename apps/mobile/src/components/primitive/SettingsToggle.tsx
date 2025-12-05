import React from "react";
import { Switch } from "./Switch";
import { SettingsRow, SettingsRowProps } from "./SettingsRow";

interface SettingsToggleProps extends Omit<SettingsRowProps, "trailing" | "onPress"> {
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
			trailing={<Switch value={value} onValueChange={onValueChange} />}
		/>
	);
}
