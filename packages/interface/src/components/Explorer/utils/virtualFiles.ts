import type { File } from "@sd/ts-client";

/**
 * Virtual File System
 *
 * Maps non-file entities (locations, volumes, devices) to the File interface
 * so they can be displayed in standard Explorer views (grid, list, column).
 * This allows reusing all existing Explorer functionality without backend changes.
 */

export type VirtualFileType = "location" | "volume" | "device";

export interface VirtualMetadata {
	type: VirtualFileType;
	data: any;
	iconUrl?: string; // Custom icon URL override
}

/**
 * Maps a Location to a File-like object for Explorer display
 */
export function mapLocationToFile(location: any, iconUrl?: string): File {
	return {
		id: `virtual:location:${location.id}`,
		kind: "Directory",
		name: location.name,
		sd_path: location.sd_path,
		size: null,
		date_created: null,
		date_modified: null,
		date_accessed: null,
		date_indexed: null,
		date_taken: null,
		has_thumbnail: false,
		checksum: null,
		hidden: false,
		favorite: false,
		important: false,
		note: null,
		entry: null,
		content: null,
		tags: [],
		labels: [],
		_virtual: {
			type: "location",
			data: location,
			iconUrl,
		} as any,
	} as unknown as File;
}

/**
 * Maps a Volume to a File-like object for Explorer display
 */
export function mapVolumeToFile(
	volume: any,
	deviceSlug: string,
	iconUrl?: string,
): File {
	const sdPath = {
		Physical: {
			device_slug: deviceSlug,
			path: volume.mount_point || "/",
		},
	};

	return {
		id: `virtual:volume:${volume.fingerprint}`,
		kind: "Directory",
		name: volume.display_name || volume.name,
		sd_path: sdPath,
		size: volume.total_bytes ? BigInt(volume.total_bytes) : null,
		date_created: null,
		date_modified: null,
		date_accessed: null,
		date_indexed: null,
		date_taken: null,
		has_thumbnail: false,
		checksum: null,
		hidden: false,
		favorite: false,
		important: false,
		note: null,
		entry: null,
		content: null,
		tags: [],
		labels: [],
		_virtual: {
			type: "volume",
			data: volume,
			iconUrl,
		} as any,
	} as unknown as File;
}

/**
 * Maps a Device to a File-like object for Explorer display
 */
export function mapDeviceToFile(device: any, iconUrl?: string): File {
	return {
		id: `virtual:device:${device.id}`,
		kind: "Directory",
		name: device.name,
		sd_path: null as any, // Devices don't have SD paths
		size: null,
		date_created: null,
		date_modified: null,
		date_accessed: null,
		date_indexed: null,
		date_taken: null,
		has_thumbnail: false,
		checksum: null,
		hidden: false,
		favorite: false,
		important: false,
		note: null,
		entry: null,
		content: null,
		tags: [],
		labels: [],
		_virtual: {
			type: "device",
			data: device,
			iconUrl,
		} as any,
	} as unknown as File;
}

/**
 * Checks if a file is a virtual file
 */
export function isVirtualFile(file: File): boolean {
	return (file as any)._virtual !== undefined;
}

/**
 * Gets virtual metadata from a file
 */
export function getVirtualMetadata(file: File): VirtualMetadata | null {
	return (file as any)._virtual || null;
}
