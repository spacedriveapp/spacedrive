import React from "react";
import { Image } from "react-native";
import { useRouter } from "expo-router";
import { useNormalizedQuery } from "../../../client";
import { SettingsGroup, SettingsLink } from "../../../components/primitive";
import FolderIcon from "@sd/assets/icons/Folder.png";
import type { Device } from "@sd/ts-client";

export function LocationsGroup() {
	const router = useRouter();
	const { data: locationsData } = useNormalizedQuery({
		wireMethod: "query:locations.list",
		input: null,
		resourceType: "location",
	});

	const { data: devices } = useNormalizedQuery<any, Device[]>({
		wireMethod: "query:devices.list",
		input: {
			include_offline: true,
			include_details: false,
			show_paired: true,
		},
		resourceType: "device",
	});

	const locations = locationsData?.locations ?? [];

	if (locations.length === 0) {
		return null;
	}

	// Helper to get device name from device_slug
	const getDeviceName = (location: any) => {
		const deviceSlug = location.sd_path?.Physical?.device_slug;
		if (!deviceSlug) return "Unknown device";
		const device = devices?.find((d) => d.slug === deviceSlug);
		return device?.name || "Unknown device";
	};

	return (
		<SettingsGroup header="Locations">
			{locations.map((location: any) => (
				<SettingsLink
					key={location.id}
					icon={
						<Image
							source={FolderIcon}
							className="w-8 h-8"
							style={{ resizeMode: "contain" }}
						/>
					}
					label={location.name || "Unnamed"}
					description={getDeviceName(location)}
					onPress={() => {
						router.push({
							pathname: "/explorer",
							params: {
								type: "path",
								path: JSON.stringify(location.sd_path),
							},
						});
					}}
				/>
			))}
		</SettingsGroup>
	);
}
