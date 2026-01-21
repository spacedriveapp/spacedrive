import type { File } from "./generated/types";

/**
 * Virtual File System
 *
 * Maps non-file entities (locations, volumes, devices) to the File interface
 * so they can be displayed in standard Explorer views (grid, list, column).
 * This allows reusing all existing Explorer functionality without backend changes.
 *
 * IMPORTANT: Virtual files should NOT be passed to file operations like copy/move/delete.
 * Always check `isVirtualFile()` before performing file operations that interact with the backend.
 * Virtual files are for display purposes only - they represent entities like locations and volumes,
 * not actual filesystem entries.
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
		extension: null,
		size: 0,
		content_identity: null,
		alternate_paths: [],
		tags: [],
		sidecars: [],
		image_media_data: null,
		video_media_data: null,
		audio_media_data: null,
		created_at: new Date().toISOString(),
		modified_at: new Date().toISOString(),
		accessed_at: null,
		content_kind: "unknown",
		is_local: true,
		duration_seconds: null,
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
		extension: null,
		size: volume.total_capacity ? Number(volume.total_capacity) : 0,
		content_identity: null,
		alternate_paths: [],
		tags: [],
		sidecars: [],
		image_media_data: null,
		video_media_data: null,
		audio_media_data: null,
		created_at: new Date().toISOString(),
		modified_at: new Date().toISOString(),
		accessed_at: null,
		content_kind: "unknown",
		is_local: true,
		duration_seconds: null,
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
	const sdPath = {
		Physical: {
			device_slug: device.slug,
			path: "/",
		},
	};

	return {
		id: `virtual:device:${device.id}`,
		kind: "Directory",
		name: device.name,
		sd_path: sdPath,
		extension: null,
		size: 0,
		content_identity: null,
		alternate_paths: [],
		tags: [],
		sidecars: [],
		image_media_data: null,
		video_media_data: null,
		audio_media_data: null,
		created_at: new Date().toISOString(),
		modified_at: new Date().toISOString(),
		accessed_at: null,
		content_kind: "unknown",
		is_local: true,
		duration_seconds: null,
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
export function isVirtualFile(file: File | undefined | null): boolean {
	return file != null && (file as any)._virtual !== undefined;
}

/**
 * Gets virtual metadata from a file
 */
export function getVirtualMetadata(
	file: File | undefined | null,
): VirtualMetadata | null {
	return file ? (file as any)._virtual || null : null;
}
