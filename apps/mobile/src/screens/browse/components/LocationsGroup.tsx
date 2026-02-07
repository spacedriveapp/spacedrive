import React from "react";
import { Image } from "react-native";
import { useRouter } from "expo-router";
import { useNormalizedQuery } from "../../../client";
import { SettingsGroup, SettingsLink } from "../../../components/primitive";
import FolderIcon from "@sd/assets/icons/Folder.png";
import type { Location, SdPath } from "@sd/ts-client";

// Extract path string and device slug from SdPath
function extractPathInfo(sdPath: SdPath): { path: string; deviceSlug: string } {
	if ("Physical" in sdPath) {
		return {
			path: sdPath.Physical.path,
			deviceSlug: sdPath.Physical.device_slug,
		};
	}
	if ("Cloud" in sdPath) {
		return {
			path: sdPath.Cloud.path,
			deviceSlug: `cloud-${sdPath.Cloud.service}`,
		};
	}
	return { path: "/", deviceSlug: "local" };
}

export function LocationsGroup() {
	const router = useRouter();
	const { data: locationsData } = useNormalizedQuery<
		any,
		{ locations: Location[] }
	>({
		query: "locations.list",
		input: null,
		resourceType: "location",
	});

	const locations = locationsData?.locations ?? [];

	if (locations.length === 0) {
		return null;
	}

	return (
		<SettingsGroup header="Locations">
			{locations.map((location) => {
				const { path, deviceSlug } = extractPathInfo(location.sd_path);
				return (
					<SettingsLink
						key={location.id}
						icon={
							<Image
								source={FolderIcon}
								className="w-6 h-6"
								style={{ resizeMode: "contain" }}
							/>
						}
						label={location.name || "Unnamed"}
						description={path || "No path"}
						onPress={() => {
							router.push({
								pathname: "/location/[locationId]",
								params: {
									locationId: location.id,
									name: location.name || "Location",
									path: path,
									deviceSlug: deviceSlug,
								},
							});
						}}
					/>
				);
			})}
		</SettingsGroup>
	);
}
