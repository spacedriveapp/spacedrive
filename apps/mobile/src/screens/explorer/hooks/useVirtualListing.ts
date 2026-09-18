import { useMemo } from "react";
import { useLibraryQuery, useNormalizedQuery } from "../../../client";
import {
	getVolumeIcon,
	getDeviceIcon,
	mapSourceToFile,
	mapVolumeToFile,
	mapDeviceToFile,
	type File,
	type Device,
	type SourceInfo,
	type Volume,
} from "@sd/ts-client";
import FolderIcon from "@sd/assets/icons/Folder.webp";

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
 * - { type: "view", view: "device", id: "device-123" }  → Sources + Volumes for that device
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
		query: "devices.list",
		input: {
			include_offline: true,
			include_details: false,
			show_paired: true,
		},
		resourceType: "device",
		enabled: isVirtualView,
	});

	// Sources, including paired devices' replicas, which carry their owning
	// device's id.
	const { data: sourcesData, isLoading: sourcesLoading } = useLibraryQuery<
		SourceInfo[]
	>("sources.list", { data_type: null }, {
		enabled: isVirtualView && view === "device",
	});

	// Fetch volumes
	const { data: volumesData, isLoading: volumesLoading } = useNormalizedQuery<
		any,
		{ volumes: Volume[] }
	>({
		query: "volumes.list",
		input: { filter: "All" },
		resourceType: "volume",
		enabled: isVirtualView && view === "device",
	});

	const files = useMemo(() => {
		if (!isVirtualView) return null;

		// View: Single device (sources + volumes for that device)
		if (view === "device" && id) {
			const device = devices?.find((d) => d.id === id);
			if (!device) return [];

			const sources = sourcesData ?? [];
			const volumes = volumesData?.volumes || [];

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

			// Filter volumes by device_id
			const deviceVolumes = volumes.filter((vol) => vol.device_id === id);

			const sourceFiles = deviceSources.map((source) =>
				mapSourceToFile(source, device.slug, FolderIcon),
			);

			const volumeFiles = deviceVolumes.map((vol) => {
				const volumeIconSrc = getVolumeIcon(vol);
				return mapVolumeToFile(vol, device.slug, volumeIconSrc);
			});

			return [...sourceFiles, ...volumeFiles];
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
	}, [isVirtualView, view, id, devices, sourcesData, volumesData]);

	return {
		files,
		isVirtualView,
		viewType: view as VirtualViewType,
		isLoading: devicesLoading || sourcesLoading || volumesLoading,
	};
}
