import React from "react";
import { Image, ImageSourcePropType } from "react-native";
import { useRouter } from "expo-router";
import { useNormalizedQuery } from "../../../client";
import type { Volume, Device } from "@sd/ts-client";
import { getVolumeIcon } from "@sd/ts-client";
import { SettingsGroup, SettingsLink } from "../../../components/primitive";

export function VolumesGroup() {
	const router = useRouter();
	const { data: volumesData } = useNormalizedQuery<any, { volumes: Volume[] }>(
		{
			query: "volumes.list",
			input: { filter: "All" },
			resourceType: "volume",
		}
	);

	const { data: devices } = useNormalizedQuery<any, Device[]>({
		query: "devices.list",
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
				// Cast volume_type for compatibility with getVolumeIcon's expected type
				const volumeIconSrc = getVolumeIcon({
					mount_point: volume.mount_point,
					volume_type: volume.volume_type as
						| "Internal"
						| "External"
						| "Removable"
						| undefined,
				}) as ImageSourcePropType;
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
							router.push({
								pathname: "/volume/[volumeId]",
								params: {
									volumeId: volume.id,
									name: volume.display_name || volume.name,
								},
							});
						}}
					/>
				);
			})}
		</SettingsGroup>
	);
}
