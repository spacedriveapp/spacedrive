import React from "react";
import { View, Image } from "react-native";
import { useRouter } from "expo-router";
import { useNormalizedQuery } from "../../../client";
import type { Device } from "@sd/ts-client";
import { getDeviceIcon } from "@sd/ts-client";
import { SettingsGroup, SettingsLink } from "../../../components/primitive";

export function DevicesGroup() {
	const router = useRouter();
	const { data: devices, isLoading } = useNormalizedQuery<any, Device[]>({
		wireMethod: "query:devices.list",
		input: {
			include_offline: true,
			include_details: false,
			show_paired: true,
		},
		resourceType: "device",
	});

	if (isLoading || !devices || devices.length === 0) {
		return null;
	}

	return (
		<SettingsGroup header="Devices">
			{devices.map((device) => {
				const deviceIconSrc = getDeviceIcon(device);
				return (
					<SettingsLink
						key={device.id}
						icon={
							<Image
								source={deviceIconSrc}
								className="w-8 h-8"
								style={{ resizeMode: "contain" }}
							/>
						}
						label={device.name}
						description={
							device.is_current
								? "This device"
								: device.is_connected
									? "Online"
									: "Offline"
						}
						onPress={() => {
							router.push({
								pathname: "/explorer",
								params: {
									type: "view",
									view: "device",
									id: device.id,
								},
							});
						}}
					/>
				);
			})}
		</SettingsGroup>
	);
}
