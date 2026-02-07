import React from "react";
import { View, Image, ImageSourcePropType } from "react-native";
import { useRouter } from "expo-router";
import { useNormalizedQuery } from "../../../client";
import type { Device } from "@sd/ts-client";
import { getDeviceIcon } from "@sd/ts-client";
import { SettingsGroup, SettingsLink } from "../../../components/primitive";

export function DevicesGroup() {
	const router = useRouter();
	const { data: devices, isLoading } = useNormalizedQuery<any, Device[]>({
		query: "devices.list",
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
				// Cast since getDeviceIcon returns imported PNG modules
				const deviceIconSrc = getDeviceIcon(device as any) as ImageSourcePropType;
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
								pathname: "/device/[deviceId]",
								params: {
									deviceId: device.id,
									name: device.name,
								},
							});
						}}
					/>
				);
			})}
		</SettingsGroup>
	);
}
