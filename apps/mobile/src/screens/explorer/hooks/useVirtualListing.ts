import { useMemo } from "react";
import { useNormalizedQuery } from "../../../client";
import {
	getVolumeIcon,
	getDeviceIcon,
	mapLocationToFile,
	mapVolumeToFile,
	mapDeviceToFile,
	type File,
	type Device,
	type Volume,
} from "@sd/ts-client";
import FolderIcon from "@sd/assets/icons/Folder.png";

export type VirtualViewType = "device" | "devices" | null;

export interface VirtualListingResult {
	files: File[] | null;
	isVirtualView: boolean;
	viewType: VirtualViewType;
	isLoading: boolean;
}

/**
 * Virtual Listing Hook (Mobile)
 *
 * Detects virtual view types from navigation params and provides mapped File[] data.
 * Supports:
 * - { type: "view", view: "device", id: "device-123" }  → Locations + Volumes for that device
 * - { type: "view", view: "devices" }                   → All devices in library
 */
export function useVirtualListing(
	params:
		| { type: "path"; path: string }
		| { type: "view"; view: string; id?: string }
		| undefined,
): VirtualListingResult {
	const isVirtualView = params?.type === "view";
	const view = isVirtualView ? params.view : null;
	const id = isVirtualView ? params.id : null;

	// Fetch devices
	const { data: devices, isLoading: devicesLoading } = useNormalizedQuery<
		any,
		Device[]
	>({
		wireMethod: "query:devices.list",
		input: {
			include_offline: true,
			include_details: false,
			show_paired: true,
		},
		resourceType: "device",
		enabled: isVirtualView,
	});

	// Fetch locations
	const { data: locationsData, isLoading: locationsLoading } =
		useNormalizedQuery({
			wireMethod: "query:locations.list",
			input: null,
			resourceType: "location",
			enabled: isVirtualView && view === "device",
		});

	// Fetch volumes
	const { data: volumesData, isLoading: volumesLoading } = useNormalizedQuery<
		any,
		{ volumes: Volume[] }
	>({
		wireMethod: "query:volumes.list",
		input: { filter: "All" },
		resourceType: "volume",
		enabled: isVirtualView && view === "device",
	});

	const files = useMemo(() => {
		if (!isVirtualView) return null;

		// View: Single device (locations + volumes for that device)
		if (view === "device" && id) {
			const device = devices?.find((d) => d.id === id);
			if (!device) return [];

			const locations = locationsData?.locations || [];
			const volumes = volumesData?.volumes || [];

			// Filter locations by device_slug
			const deviceLocations = locations.filter(
				(loc: any) => loc.sd_path?.Physical?.device_slug === device.slug,
			);

			// Filter volumes by device_id
			const deviceVolumes = volumes.filter((vol) => vol.device_id === id);

			const locationFiles = deviceLocations.map((loc: any) =>
				mapLocationToFile(loc, FolderIcon),
			);

			const volumeFiles = deviceVolumes.map((vol) => {
				const volumeIconSrc = getVolumeIcon(vol);
				return mapVolumeToFile(vol, device.slug, volumeIconSrc);
			});

			return [...locationFiles, ...volumeFiles];
		}

		// View: All devices
		if (view === "devices") {
			if (!devices) return [];

			return devices.map((device) => {
				const deviceIconSrc = getDeviceIcon(device);
				return mapDeviceToFile(device, deviceIconSrc);
			});
		}

		return [];
	}, [isVirtualView, view, id, devices, locationsData, volumesData]);

	return {
		files,
		isVirtualView,
		viewType: view as VirtualViewType,
		isLoading: devicesLoading || locationsLoading || volumesLoading,
	};
}
