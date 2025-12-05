import React from "react";
import { View } from "react-native";
import { SettingsRow, SettingsRowProps } from "./SettingsRow";

type SettingsLinkProps = Omit<SettingsRowProps, "trailing">;

export function SettingsLink(props: SettingsLinkProps) {
	return (
		<SettingsRow
			{...props}
			trailing={
				<View className="w-2 h-2 border-r-2 border-t-2 border-ink-dull rotate-45" />
			}
		/>
	);
}
