import type { Icon } from "@phosphor-icons/react";
import {
  Clock,
  Folders,
  HardDrive,
  Heart,
  House,
  Tag as TagIcon,
} from "@phosphor-icons/react";
import { Location } from "@sd/assets/icons";
import type {
  File,
  ItemType,
  SdPath,
  SpaceItem as SpaceItemType,
} from "@sd/ts-client";

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

export function isLocationItem(
  t: ItemType
): t is { Location: { location_id: string } } {
  return typeof t === "object" && "Location" in t;
}

export function isVolumeItem(
  t: ItemType
): t is { Volume: { volume_id: string } } {
  return typeof t === "object" && "Volume" in t;
}

export function isTagItem(t: ItemType): t is { Tag: { tag_id: string } } {
  return typeof t === "object" && "Tag" in t;
}

export function isPathItem(t: ItemType): t is { Path: { sd_path: SdPath } } {
  return typeof t === "object" && "Path" in t;
}

// Check if item is a "raw" location (legacy format with name/sd_path but no item_type)
export function isRawLocation(
  item: SpaceItemType | Record<string, unknown>
): boolean {
  return "name" in item && "sd_path" in item && !("item_type" in item);
}

// Get icon data for an item type
function getItemIcon(itemType: ItemType): IconData {
  if (isOverviewItem(itemType)) return { type: "component", icon: House };
  if (isRecentsItem(itemType)) return { type: "component", icon: Clock };
  if (isFavoritesItem(itemType)) return { type: "component", icon: Heart };
  if (isFileKindsItem(itemType)) return { type: "component", icon: Folders };
  if (isLocationItem(itemType)) return { type: "image", icon: Location };
  if (isVolumeItem(itemType)) return { type: "component", icon: HardDrive };
  if (isTagItem(itemType)) return { type: "component", icon: TagIcon };
  if (isPathItem(itemType)) return { type: "image", icon: Location };
  return { type: "image", icon: Location };
}

// Get label for an item type
function getItemLabel(itemType: ItemType, resolvedFile?: File | null): string {
  if (isOverviewItem(itemType)) return "Overview";
  if (isRecentsItem(itemType)) return "Recents";
  if (isFavoritesItem(itemType)) return "Favorites";
  if (isFileKindsItem(itemType)) return "File Kinds";
  if (isLocationItem(itemType))
    return itemType.Location.name || "Unnamed Location";
  if (isVolumeItem(itemType)) return itemType.Volume.name || "Unnamed Volume";
  if (isTagItem(itemType)) return itemType.Tag.name || "Unnamed Tag";
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
  return "Unknown";
}

// Build navigation path for an item
function getItemPath(
  itemType: ItemType,
  volumeData?: { device_slug: string; mount_path: string },
  itemSdPath?: SdPath
): string | null {
  if (isOverviewItem(itemType)) return "/";
  if (isRecentsItem(itemType)) return "/recents";
  if (isFavoritesItem(itemType)) return "/favorites";
  if (isFileKindsItem(itemType)) return "/file-kinds";

  if (isLocationItem(itemType)) {
    // Use explorer route with location's SD path (passed from item.sd_path)
    if (itemSdPath) {
      return `/explorer?path=${encodeURIComponent(JSON.stringify(itemSdPath))}`;
    }
    return null;
  }

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
  item: SpaceItemType | Record<string, unknown>,
  options: ResolveMetadataOptions = {}
): ItemMetadata {
  const { volumeData, customIcon, customLabel } = options;

  // Handle raw location object (legacy format)
  if (isRawLocation(item)) {
    const rawItem = item as { name?: string; sd_path?: SdPath };
    const label = customLabel || rawItem.name || "Unnamed Location";
    const path = rawItem.sd_path
      ? `/explorer?path=${encodeURIComponent(JSON.stringify(rawItem.sd_path))}`
      : null;

    return {
      icon: customIcon
        ? { type: "image", icon: customIcon }
        : { type: "image", icon: Location },
      label,
      path,
    };
  }

  // Handle proper SpaceItem
  const spaceItem = item as SpaceItemType;
  const resolvedFile = spaceItem.resolved_file;
  const itemSdPath = (spaceItem as SpaceItemType & { sd_path?: SdPath })
    .sd_path;

  const icon: IconData = customIcon
    ? { type: "image", icon: customIcon }
    : getItemIcon(spaceItem.item_type);

  const label =
    customLabel ||
    resolvedFile?.name ||
    getItemLabel(spaceItem.item_type, resolvedFile);

  const path = getItemPath(spaceItem.item_type, volumeData, itemSdPath);

  return { icon, label, path };
}

// Determine if an item can be a drop target (for files to be moved into)
export function isDropTargetItem(
  item: SpaceItemType | Record<string, unknown>
): boolean {
  if (isRawLocation(item)) return true;

  const spaceItem = item as SpaceItemType;
  const itemType = spaceItem.item_type;
  const resolvedFile = spaceItem.resolved_file;

  return (
    isLocationItem(itemType) ||
    isVolumeItem(itemType) ||
    (isPathItem(itemType) && resolvedFile?.kind === "Directory")
  );
}

// Get the target type for drop operations
export type DropTargetType = "location" | "volume" | "folder" | "other";

export function getDropTargetType(
  item: SpaceItemType | Record<string, unknown>
): DropTargetType {
  if (isRawLocation(item)) return "location";

  const spaceItem = item as SpaceItemType;
  const itemType = spaceItem.item_type;
  const resolvedFile = spaceItem.resolved_file;

  if (isLocationItem(itemType)) return "location";
  if (isVolumeItem(itemType)) return "volume";
  if (isPathItem(itemType) && resolvedFile?.kind === "Directory")
    return "folder";

  return "other";
}

// Build target path for drop operations
export function buildDropTargetPath(
  item: SpaceItemType | Record<string, unknown>,
  volumeData?: { device_slug: string; mount_path: string }
): SdPath | undefined {
  if (isRawLocation(item)) {
    return (item as { sd_path?: SdPath }).sd_path;
  }

  const spaceItem = item as SpaceItemType;
  const itemType = spaceItem.item_type;
  const itemSdPath = (spaceItem as SpaceItemType & { sd_path?: SdPath })
    .sd_path;

  if (isPathItem(itemType)) {
    return itemType.Path.sd_path;
  }

  if (isVolumeItem(itemType) && volumeData) {
    return {
      Physical: {
        device_slug: volumeData.device_slug,
        path: volumeData.mount_path || "/",
      },
    };
  }

  if (isLocationItem(itemType) && itemSdPath) {
    return itemSdPath;
  }

  return undefined;
}
