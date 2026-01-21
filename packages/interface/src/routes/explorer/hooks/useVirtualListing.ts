import { useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import { useNormalizedQuery, getDeviceIcon } from "../../../contexts/SpacedriveContext";
import {
	getVolumeIcon,
	mapLocationToFile,
	mapVolumeToFile,
	mapDeviceToFile,
	type File,
} from "@sd/ts-client";
import { Location } from "@sd/assets/icons";

export type VirtualViewType = "device" | "devices" | null;

export interface VirtualListingResult {
	files: File[] | null;
	isVirtualView: boolean;
	viewType: VirtualViewType;
	isLoading: boolean;
}

/**
 * Virtual Listing Hook
 *
 * Detects virtual view types from URL params and provides mapped File[] data.
 * Supports:
 * - ?view=device&id=device-123  → Locations + Volumes for that device
 * - ?view=devices               → All devices in library
 */
export function useVirtualListing(): VirtualListingResult {
	const [searchParams] = useSearchParams();
	const view = searchParams.get("view") as VirtualViewType;
	const deviceId = searchParams.get("id");

	const isVirtualView = view !== null;

	// Fetch locations
	const { data: locationsData, isLoading: locationsLoading } =
		useNormalizedQuery({
			wireMethod: "query:locations.list",
			input: null,
			resourceType: "location",
			enabled: view === "device",
		});

	// Fetch volumes
	const { data: volumesData, isLoading: volumesLoading } = useNormalizedQuery(
		{
			wireMethod: "query:volumes.list",
			input: { filter: "All" },
			resourceType: "volume",
			enabled: view === "device",
		},
	);

	// Fetch devices
	const { data: devicesData, isLoading: devicesLoading } = useNormalizedQuery(
		{
			wireMethod: "query:devices.list",
			input: { include_offline: true, include_details: false },
			resourceType: "device",
			enabled: view === "devices" || view === "device",
		},
	);

	const files = useMemo(() => {
		if (!isVirtualView) return null;

		// View: All Devices
		if (view === "devices") {
			const devices = (devicesData as any[]) || [];
			return devices.map((device) =>
				mapDeviceToFile(device, getDeviceIcon(device)),
			);
		}

		// View: Single Device (Locations + Volumes)
		if (view === "device" && deviceId) {
			const locations = (locationsData as any)?.locations || [];
			const volumes = (volumesData as any)?.volumes || [];
			const devices = (devicesData as any[]) || [];

			const device = devices.find((d: any) => d.id === deviceId);
			if (!device) return [];

			const virtualFiles: File[] = [];

			// Add locations for this device
			// Filter locations that belong to this device (match by device in sd_path)
			const deviceLocations = locations.filter((loc: any) => {
				if (!loc.sd_path || !("Physical" in loc.sd_path)) return false;
				return loc.sd_path.Physical.device_slug === device.slug;
			});

			virtualFiles.push(
				...deviceLocations.map((loc: any) =>
					mapLocationToFile(loc, Location),
				),
			);

			// Add volumes for this device
			const deviceVolumes = volumes.filter(
				(vol: any) => vol.device_id === device.id,
			);

			virtualFiles.push(
				...deviceVolumes.map((vol: any) =>
					mapVolumeToFile(vol, device.slug, getVolumeIcon(vol)),
				),
			);

			return virtualFiles;
		}

		return [];
	}, [
		view,
		deviceId,
		locationsData,
		volumesData,
		devicesData,
		isVirtualView,
	]);

	const isLoading =
		(view === "device" &&
			(locationsLoading || volumesLoading || devicesLoading)) ||
		(view === "devices" && devicesLoading);

	return {
		files,
		isVirtualView,
		viewType: view,
		isLoading,
	};
}