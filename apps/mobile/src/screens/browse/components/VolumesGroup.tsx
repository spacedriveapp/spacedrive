import React from "react";
import { Image } from "react-native";
import { useRouter } from "expo-router";
import { useNormalizedQuery } from "../../../client";
import type { Volume, Device } from "@sd/ts-client";
import { getVolumeIcon } from "@sd/ts-client";
import { SettingsGroup, SettingsLink } from "../../../components/primitive";

export function VolumesGroup() {
	const router = useRouter();
	const { data: volumesData } = useNormalizedQuery<any, { volumes: Volume[] }>(
		{
			wireMethod: "query:volumes.list",
			input: { filter: "All" },
			resourceType: "volume",
		}
	);

	const { data: devices } = useNormalizedQuery<any, Device[]>({
		wireMethod: "query:devices.list",
		input: {
			include_offline: true,
			include_details: false,
			show_paired: true,
		},
		resourceType: "device",
	});

	const volumes = volumesData?.volumes || [];

	if (volumes.length === 0) {
		return null;
	}

	return (
		<SettingsGroup header="Volumes">
			{volumes.map((volume) => {
				const volumeIconSrc = getVolumeIcon(volume);
				const device = devices?.find((d) => d.id === volume.device_id);

				return (
					<SettingsLink
						key={volume.id}
						icon={
							<Image
								source={volumeIconSrc}
								className="w-8 h-8"
								style={{ resizeMode: "contain" }}
							/>
						}
						label={volume.display_name || volume.name}
						description={
							volume.is_tracked ? "Tracked" : "Not tracked"
						}
						onPress={() => {
							if (device) {
								const sdPath = {
									Physical: {
										device_slug: device.slug,
										path: volume.mount_point || "/",
									},
								};
								router.push({
									pathname: "/explorer",
									params: {
										type: "path",
										path: JSON.stringify(sdPath),
									},
								});
							}
						}}
					/>
				);
			})}
		</SettingsGroup>
	);
}
