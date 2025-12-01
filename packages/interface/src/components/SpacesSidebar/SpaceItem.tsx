import { useNavigate, useLocation } from "react-router-dom";
import clsx from "clsx";
import {
	House,
	Clock,
	Heart,
	Folder,
	HardDrive,
	Tag as TagIcon,
} from "@phosphor-icons/react";
import { Location } from "@sd/assets/icons";
import type {
	SpaceItem as SpaceItemType,
	ItemType,
	File,
} from "@sd/ts-client";
import { Thumb } from "../Explorer/File/Thumb";

interface SpaceItemProps {
	item: SpaceItemType;
	/** Optional component to render on the right side (e.g., badges, status indicators) */
	rightComponent?: React.ReactNode;
	/** Optional className to override default styling */
	className?: string;
	/** Optional icon weight (default: "bold") */
	iconWeight?: "thin" | "light" | "regular" | "bold" | "fill" | "duotone";
	/** Optional onClick handler to override default navigation */
	onClick?: () => void;
	/** Volume data for constructing explorer path */
	volumeData?: { device_slug: string; mount_path: string };
	/** Optional custom icon (as image path) to override default icon */
	customIcon?: string;
}

function getItemIcon(itemType: ItemType): any {
	if (itemType === "Overview") return { type: "component", icon: House };
	if (itemType === "Recents") return { type: "component", icon: Clock };
	if (itemType === "Favorites") return { type: "component", icon: Heart };
	if (typeof itemType === "object" && "Location" in itemType)
		return { type: "image", icon: Location };
	if (typeof itemType === "object" && "Volume" in itemType)
		return { type: "component", icon: HardDrive };
	if (typeof itemType === "object" && "Tag" in itemType)
		return { type: "component", icon: TagIcon };
	if (typeof itemType === "object" && "Path" in itemType)
		return { type: "image", icon: Location };
	return { type: "image", icon: Location };
}

function getItemLabel(itemType: ItemType): string {
	if (itemType === "Overview") return "Overview";
	if (itemType === "Recents") return "Recents";
	if (itemType === "Favorites") return "Favorites";
	if (typeof itemType === "object" && "Location" in itemType) {
		return itemType.Location.name || "Unnamed Location";
	}
	if (typeof itemType === "object" && "Volume" in itemType) {
		return itemType.Volume.name || "Unnamed Volume";
	}
	if (typeof itemType === "object" && "Tag" in itemType) {
		return itemType.Tag.name || "Unnamed Tag";
	}
	if (typeof itemType === "object" && "Path" in itemType) {
		// Extract name from path
		const path = itemType.Path.sd_path;
		if (typeof path === "object" && "Physical" in path) {
			const parts = path.Physical.path.split("/");
			return parts[parts.length - 1] || "Path";
		}
		return "Path";
	}
	return "Unknown";
}

function getItemPath(itemType: ItemType, volumeData?: { device_slug: string; mount_path: string }): string | null {
	if (itemType === "Overview") return "/";
	if (itemType === "Recents") return "/recents";
	if (itemType === "Favorites") return "/favorites";
	if (typeof itemType === "object" && "Location" in itemType)
		return `/location/${itemType.Location.location_id}`;
	if (typeof itemType === "object" && "Volume" in itemType) {
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
		return `/volume/${itemType.Volume.volume_id}`;
	}
	if (typeof itemType === "object" && "Tag" in itemType)
		return `/tag/${itemType.Tag.tag_id}`;
	return null;
}

export function SpaceItem({
	item,
	rightComponent,
	className,
	iconWeight = "bold",
	onClick,
	volumeData,
	customIcon,
}: SpaceItemProps) {
	const navigate = useNavigate();
	const location = useLocation();

	// Check if this is a raw location object (has 'name' and 'sd_path' but no 'item_type')
	const isRawLocation =
		"name" in item && "sd_path" in item && !item.item_type;

	// Check if we have a resolved file
	const resolvedFile = item.resolved_file as File | undefined;

	let iconData, label, path;

	if (isRawLocation) {
		// Handle raw location object
		iconData = { type: "image", icon: Location };
		label = (item as any).name || "Unnamed Location";
		path = `/location/${item.id}`;
	} else {
		// Handle proper SpaceItem
		iconData = getItemIcon(item.item_type);
		// Use resolved file name if available, otherwise parse from item_type
		label = resolvedFile?.name || getItemLabel(item.item_type);
		path = getItemPath(item.item_type, volumeData);
	}

	// Override with custom icon if provided
	if (customIcon) {
		iconData = { type: "image", icon: customIcon };
	}

	// Check if this item is active
	// For paths with query params (like volumes), compare full path including search
	const isActive = path
		? path.includes("?")
			? location.pathname + location.search === path
			: location.pathname === path
		: false;

	const handleClick = () => {
		if (onClick) {
			onClick();
		} else if (path) {
			navigate(path);
		}
	};

	return (
		<button
			onClick={handleClick}
			className={clsx(
				"flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm font-medium",
				className ||
					(isActive
						? "bg-sidebar-selected/30 text-sidebar-ink"
						: "text-sidebar-inkDull"),
			)}
		>
			{resolvedFile ? (
				<Thumb file={resolvedFile} size={16} className="shrink-0" />
			) : iconData.type === "image" ? (
				<img src={iconData.icon} alt="" className="size-4" />
			) : (
				<iconData.icon className="size-4" weight={iconWeight} />
			)}
			<span className="flex-1 truncate text-left">{label}</span>
			{rightComponent}
		</button>
	);
}
