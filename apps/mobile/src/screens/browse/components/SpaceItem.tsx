import React from "react";
import { Image } from "react-native";
import { useRouter } from "expo-router";
import type { SpaceItem as SpaceItemType, ItemType, SdPath } from "@sd/ts-client";
import { SettingsLink } from "../../../components/primitive";
import FolderIcon from "@sd/assets/icons/Folder.webp";
import { MagnifyingGlass, Clock, Heart, Folders, HardDrive, Tag } from "phosphor-react-native";

// Type guards
function isOverviewItem(t: ItemType): t is "Overview" {
	return t === "Overview";
}

function isRecentsItem(t: ItemType): t is "Recents" {
	return t === "Recents";
}

function isFavoritesItem(t: ItemType): t is "Favorites" {
	return t === "Favorites";
}

function isFileKindsItem(t: ItemType): t is "FileKinds" {
	return t === "FileKinds";
}

function isVolumeItem(t: ItemType): t is { Volume: { volume_id: string } } {
	return typeof t === "object" && "Volume" in t;
}

function isTagItem(t: ItemType): t is { Tag: { tag_id: string } } {
	return typeof t === "object" && "Tag" in t;
}

function isPathItem(t: ItemType): t is { Path: { sd_path: SdPath } } {
	return typeof t === "object" && "Path" in t;
}

// Get icon for item type
function getItemIcon(itemType: ItemType): React.ReactNode {
	if (isOverviewItem(itemType)) {
		return <MagnifyingGlass size={20} color="hsl(235, 10%, 55%)" weight="bold" />;
	}
	if (isRecentsItem(itemType)) {
		return <Clock size={20} color="hsl(235, 10%, 55%)" weight="bold" />;
	}
	if (isFavoritesItem(itemType)) {
		return <Heart size={20} color="hsl(235, 10%, 55%)" weight="bold" />;
	}
	if (isFileKindsItem(itemType)) {
		return <Folders size={20} color="hsl(235, 10%, 55%)" weight="bold" />;
	}
	if (isPathItem(itemType)) {
		return (
			<Image
				source={FolderIcon}
				className="w-5 h-5"
				style={{ resizeMode: "contain" }}
			/>
		);
	}
	if (isVolumeItem(itemType)) {
		return <HardDrive size={20} color="hsl(235, 10%, 55%)" weight="bold" />;
	}
	if (isTagItem(itemType)) {
		return <Tag size={20} color="hsl(235, 10%, 55%)" weight="bold" />;
	}
	return (
		<Image
			source={FolderIcon}
			className="w-5 h-5"
			style={{ resizeMode: "contain" }}
		/>
	);
}

// Get label for item type
function getItemLabel(itemType: ItemType, resolvedFile?: any): string {
	if (isOverviewItem(itemType)) return "Overview";
	if (isRecentsItem(itemType)) return "Recents";
	if (isFavoritesItem(itemType)) return "Favorites";
	if (isFileKindsItem(itemType)) return "File Kinds";
	if (isVolumeItem(itemType)) return itemType.Volume.name || "Unnamed Volume";
	if (isTagItem(itemType)) return itemType.Tag.name || "Unnamed Tag";
	if (isPathItem(itemType)) {
		if (resolvedFile?.name) return resolvedFile.name;
		const sdPath = itemType.Path.sd_path;
		if (typeof sdPath === "object" && "Physical" in sdPath) {
			const parts = (sdPath as { Physical: { path: string } }).Physical.path.split("/");
			return parts[parts.length - 1] || "Path";
		}
		return "Path";
	}
	return "Unknown";
}

// Get navigation params for item (mobile uses different format)
function getItemNavigation(itemType: ItemType): { pathname: string; params?: any } | null {
	if (isOverviewItem(itemType)) {
		return { pathname: "/" };
	}
	if (isRecentsItem(itemType)) {
		return { pathname: "/recents" };
	}
	if (isFavoritesItem(itemType)) {
		return { pathname: "/favorites" };
	}
	if (isFileKindsItem(itemType)) {
		return { pathname: "/file-kinds" };
	}

	if (isVolumeItem(itemType)) {
		// Volume navigation handled separately
		return null;
	}

	if (isTagItem(itemType)) {
		return {
			pathname: "/explorer",
			params: {
				type: "view",
				view: "tag",
				id: itemType.Tag.tag_id,
			},
		};
	}

	if (isPathItem(itemType)) {
		return {
			pathname: "/explorer",
			params: {
				type: "path",
				path: JSON.stringify(itemType.Path.sd_path),
			},
		};
	}

	return null;
}

interface SpaceItemProps {
	item: SpaceItemType;
}

export function SpaceItem({ item }: SpaceItemProps) {
	const router = useRouter();

	// Normal space item
	const itemType = item.item_type;
	const icon = getItemIcon(itemType);
	const label = getItemLabel(itemType, item.resolved_file);
	const navigation = getItemNavigation(itemType);

	// Handle volume items specially
	if (isVolumeItem(itemType)) {
		// Volumes are handled by VolumesGroup component
		return null;
	}

	if (!navigation) {
		return null;
	}

	return (
		<SettingsLink
			icon={icon}
			label={label}
			onPress={() => {
				router.push(navigation);
			}}
		/>
	);
}
