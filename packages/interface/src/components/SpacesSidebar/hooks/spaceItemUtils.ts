import {
	House,
	Clock,
	Camera as CameraIcon,
	ChartPieSlice,
	Heart,
	HardDrive,
	Tag as TagIcon,
	Folders,
	Database,
	ShieldCheck,
} from "@phosphor-icons/react";
import { Folder } from "@sd/assets/icons";
import type {
	SpaceItem as SpaceItemType,
	ItemType,
	File,
	SdPath,
} from "@sd/ts-client";
import type { Icon } from "@phosphor-icons/react";

// Icon data returned from metadata resolution
export type IconData =
	| { type: "component"; icon: Icon }
	| { type: "image"; icon: string };

// Metadata resolved for a space item
export interface ItemMetadata {
	icon: IconData;
	label: string;
	path: string | null;
}

// Type guards for ItemType discrimination
export function isOverviewItem(t: ItemType): t is "Overview" {
	return t === "Overview";
}

export function isRecentsItem(t: ItemType): t is "Recents" {
	return t === "Recents";
}

export function isFavoritesItem(t: ItemType): t is "Favorites" {
	return t === "Favorites";
}

export function isFileKindsItem(t: ItemType): t is "FileKinds" {
	return t === "FileKinds";
}

export function isVolumeItem(
	t: ItemType,
): t is { Volume: { volume_id: string } } {
	return typeof t === "object" && "Volume" in t;
}

export function isTagItem(t: ItemType): t is { Tag: { tag_id: string } } {
	return typeof t === "object" && "Tag" in t;
}

export function isPathItem(t: ItemType): t is { Path: { sd_path: SdPath } } {
	return typeof t === "object" && "Path" in t;
}

export function isSourcesItem(t: ItemType): t is "Sources" {
	return t === "Sources";
}

export function isRedundancyItem(t: ItemType): t is "Redundancy" {
	return t === "Redundancy";
}

export function isAnalyzerItem(t: ItemType): t is "Analyzer" {
	return t === "Analyzer";
}

export function isCollectionItem(
	t: ItemType,
): t is { Collection: { slug: string } } {
	return typeof t === "object" && "Collection" in t;
}

export function isSourceItem(
	t: ItemType,
): t is { Source: { source_id: string } } {
	return typeof t === "object" && "Source" in t;
}

// Get icon data for an item type
function getItemIcon(itemType: ItemType): IconData {
	if (isOverviewItem(itemType)) return { type: "component", icon: House };
	if (isRecentsItem(itemType)) return { type: "component", icon: Clock };
	if (isFavoritesItem(itemType)) return { type: "component", icon: Heart };
	if (isFileKindsItem(itemType)) return { type: "component", icon: Folders };
	if (isSourcesItem(itemType)) return { type: "component", icon: Database };
	if (isRedundancyItem(itemType)) return { type: "component", icon: ShieldCheck };
	if (isCollectionItem(itemType)) return { type: "component", icon: CameraIcon };
	if (isAnalyzerItem(itemType)) return { type: "component", icon: ChartPieSlice };
	if (isVolumeItem(itemType)) return { type: "component", icon: HardDrive };
	if (isTagItem(itemType)) return { type: "component", icon: TagIcon };
	if (isPathItem(itemType)) return { type: "image", icon: Folder };
	if (isSourceItem(itemType)) return { type: "component", icon: Database };
	return { type: "image", icon: Folder };
}

// Get label for an item type
function getItemLabel(itemType: ItemType, resolvedFile?: File | null): string {
	if (isOverviewItem(itemType)) return "Overview";
	if (isRecentsItem(itemType)) return "Recents";
	if (isFavoritesItem(itemType)) return "Favorites";
	if (isFileKindsItem(itemType)) return "File Kinds";
	if (isSourcesItem(itemType)) return "Sources";
	if (isRedundancyItem(itemType)) return "Redundancy";
	if (isAnalyzerItem(itemType)) return "Analyzer";
	if (isCollectionItem(itemType)) {
		const slug = itemType.Collection.slug;
		return slug
			.split("-")
			.map((w) => w.charAt(0).toUpperCase() + w.slice(1))
			.join(" ");
	}
	if (isVolumeItem(itemType)) return resolvedFile?.name || (itemType as { Volume: { volume_id: string; name?: string } }).Volume.name || "Unnamed Volume";
	if (isTagItem(itemType)) return resolvedFile?.name || "Unnamed Tag";
	if (isPathItem(itemType)) {
		// Use resolved file name if available, otherwise extract from path
		if (resolvedFile?.name) return resolvedFile.name;
		const sdPath = itemType.Path.sd_path;
		if (typeof sdPath === "object" && "Physical" in sdPath) {
			const parts = (
				sdPath as { Physical: { path: string } }
			).Physical.path.split("/");
			return parts[parts.length - 1] || "Path";
		}
		return "Path";
	}
	if (isSourceItem(itemType)) return "Source";
	return "Unknown";
}

// Build navigation path for an item
function getItemPath(
	itemType: ItemType,
	volumeData?: { device_slug: string; mount_path: string },
): string | null {
	if (isOverviewItem(itemType)) return "/";
	if (isRecentsItem(itemType)) return "/collection/recent";
	if (isFavoritesItem(itemType)) return "/favorites";
	if (isFileKindsItem(itemType)) return "/file-kinds";
	if (isSourcesItem(itemType)) return "/sources";
	if (isRedundancyItem(itemType)) return "/redundancy";
	if (isCollectionItem(itemType))
		return `/collection/${itemType.Collection.slug}`;
	if (isAnalyzerItem(itemType)) return "/analyzer";

	if (isVolumeItem(itemType)) {
		// Navigate to explorer with volume's root path
		if (volumeData) {
			const sdPath = {
				Physical: {
					device_slug: volumeData.device_slug,
					path: volumeData.mount_path || "/",
				},
			};
			return `/explorer?path=${encodeURIComponent(JSON.stringify(sdPath))}`;
		}
		return null;
	}

	if (isTagItem(itemType)) {
		return `/tag/${itemType.Tag.tag_id}`;
	}

	if (isPathItem(itemType)) {
		// Navigate to explorer with the SD path
		return `/explorer?path=${encodeURIComponent(JSON.stringify(itemType.Path.sd_path))}`;
	}

	if (isSourceItem(itemType)) {
		return `/sources/${itemType.Source.source_id}`;
	}

	return null;
}

// Options for resolving item metadata
export interface ResolveMetadataOptions {
	volumeData?: { device_slug: string; mount_path: string };
	customIcon?: string;
	customLabel?: string;
}

// Resolve all metadata for a space item in one call
export function resolveItemMetadata(
	item: SpaceItemType,
	options: ResolveMetadataOptions = {},
): ItemMetadata {
	const { volumeData, customIcon, customLabel } = options;
	const resolvedFile = item.resolved_file;

	const icon: IconData = customIcon
		? { type: "image", icon: customIcon }
		: getItemIcon(item.item_type);

	const label =
		customLabel ||
		resolvedFile?.name ||
		getItemLabel(item.item_type, resolvedFile);

	const path = getItemPath(item.item_type, volumeData);

	return { icon, label, path };
}
