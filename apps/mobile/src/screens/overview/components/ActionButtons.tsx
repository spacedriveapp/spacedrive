import React from "react";
import { View, Image } from "react-native";
import MobileIcon from "@sd/assets/icons/Mobile.png";
import CloudSyncIcon from "@sd/assets/icons/CloudSync.png";
import NewLocationIcon from "@sd/assets/icons/NewLocation.png";
import { SettingsGroup, SettingsLink } from "../../../components/primitive";

interface ActionButtonsProps {
	onPairDevice: () => void;
	onSetupSync: () => void;
	onAddStorage: () => void;
}

export function ActionButtons({
	onPairDevice,
	onSetupSync,
	onAddStorage,
}: ActionButtonsProps) {
	return (
		<View className="mb-6">
			<SettingsGroup header="Quick Actions">
				<SettingsLink
					icon={
						<Image
							source={MobileIcon}
							className="w-6 h-6"
							style={{ resizeMode: "contain" }}
						/>
					}
					label="Pair New Device"
					description="Connect another device to share files"
					onPress={onPairDevice}
				/>
				<SettingsLink
					icon={
						<Image
							source={CloudSyncIcon}
							className="w-6 h-6"
							style={{ resizeMode: "contain" }}
						/>
					}
					label="Setup Sync"
					description="Enable cloud backup and sync"
					onPress={onSetupSync}
				/>
				<SettingsLink
					icon={
						<Image
							source={NewLocationIcon}
							className="w-6 h-6"
							style={{ resizeMode: "contain" }}
						/>
					}
					label="Add Storage"
					description="Add a new location to index"
					onPress={onAddStorage}
				/>
			</SettingsGroup>
		</View>
	);
}
