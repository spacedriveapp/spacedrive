import { useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import {
	useLibraryQuery,
	useNormalizedQuery,
	getDeviceIcon,
} from "../../../contexts/SpacedriveContext";
import {
	getVolumeIcon,
	mapSourceToFile,
	mapVolumeToFile,
	mapDeviceToFile,
	type File,
	type SourceInfo,
} from "@sd/ts-client";
import { Folder } from "@sd/assets/icons";

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
 * - ?view=device&id=device-123  → Sources + Volumes for that device
 * - ?view=devices               → All devices in library
 */
export function useVirtualListing(): VirtualListingResult {
	const [searchParams] = useSearchParams();
	const view = searchParams.get("view") as VirtualViewType;
	const deviceId = searchParams.get("id");

	const isVirtualView = view !== null;

	// Sources, including paired devices' replicas, which carry their owning
	// device's id.
	const { data: sourcesData, isLoading: sourcesLoading } = useLibraryQuery(
		{ type: "sources.list", input: { data_type: null } },
		{ enabled: view === "device" },
	);

	// Fetch volumes
	const { data: volumesData, isLoading: volumesLoading } = useNormalizedQuery(
		{
			query: "volumes.list",
			input: { filter: "All" },
			resourceType: "volume",
			enabled: view === "device",
		},
	);

	// Fetch devices
	const { data: devicesData, isLoading: devicesLoading } = useNormalizedQuery(
		{
			query: "devices.list",
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

		// View: Single Device (Sources + Volumes)
		if (view === "device" && deviceId) {
			const sources: SourceInfo[] = sourcesData ? [...sourcesData] : [];
			const volumes = (volumesData as any)?.volumes || [];
			const devices = (devicesData as any[]) || [];

			const device = devices.find((d: any) => d.id === deviceId);
			if (!device) return [];

			const virtualFiles: File[] = [];

			// A local source carries no device id; a replica carries its
			// owner's.
			const deviceSources = sources.filter(
				(source) =>
					source.data_type === "filesystem" &&
					source.root &&
					(source.device_id
						? source.device_id === device.id
						: device.is_current),
			);

			virtualFiles.push(
				...deviceSources.map((source) =>
					mapSourceToFile(source, device.slug, Folder),
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
		sourcesData,
		volumesData,
		devicesData,
		isVirtualView,
	]);

	const isLoading =
		(view === "device" &&
			(sourcesLoading || volumesLoading || devicesLoading)) ||
		(view === "devices" && devicesLoading);

	return {
		files,
		isVirtualView,
		viewType: view,
		isLoading,
	};
}