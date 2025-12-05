import React from "react";
import { Text } from "react-native";
import { SettingsRow, SettingsRowProps } from "./SettingsRow";

interface SettingsOptionProps extends SettingsRowProps {
	value?: string;
}

export function SettingsOption({ value, ...props }: SettingsOptionProps) {
	return (
		<SettingsRow
			{...props}
			trailing={
				value ? (
					<Text className="text-ink-dull mr-2">{value}</Text>
				) : undefined
			}
		/>
	);
}
