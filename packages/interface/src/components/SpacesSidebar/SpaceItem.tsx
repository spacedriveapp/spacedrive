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
} from "@sd/ts-client/generated/types";

interface SpaceItemProps {
	item: SpaceItemType;
	/** Optional component to render on the right side (e.g., badges, status indicators) */
	rightComponent?: React.ReactNode;
	/** Optional className to override default styling */
	className?: string;
	/** Optional icon weight (default: "bold") */
	iconWeight?: "thin" | "light" | "regular" | "bold" | "fill" | "duotone";
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

function getItemPath(itemType: ItemType): string | null {
	if (itemType === "Overview") return "/";
	if (itemType === "Recents") return "/recents";
	if (itemType === "Favorites") return "/favorites";
	if (typeof itemType === "object" && "Location" in itemType)
		return `/location/${itemType.Location.location_id}`;
	if (typeof itemType === "object" && "Volume" in itemType)
		return `/volume/${itemType.Volume.volume_id}`;
	if (typeof itemType === "object" && "Tag" in itemType)
		return `/tag/${itemType.Tag.tag_id}`;
	return null;
}

export function SpaceItem({
	item,
	rightComponent,
	className,
	iconWeight = "bold",
}: SpaceItemProps) {
	const navigate = useNavigate();
	const location = useLocation();

	// Check if this is a raw location object (has 'name' and 'sd_path' but no 'item_type')
	const isRawLocation =
		"name" in item && "sd_path" in item && !item.item_type;

	let iconData, label, path;

	if (isRawLocation) {
		// Handle raw location object
		iconData = { type: "image", icon: Location };
		label = (item as any).name || "Unnamed Location";
		path = `/location/${item.id}`;
	} else {
		// Handle proper SpaceItem
		iconData = getItemIcon(item.item_type);
		label = getItemLabel(item.item_type);
		path = getItemPath(item.item_type);
	}

	const isActive = location.pathname === path;

	const handleClick = () => {
		if (path) {
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
			{iconData.type === "image" ? (
				<img src={iconData.icon} alt="" className="size-4" />
			) : (
				<iconData.icon className="size-4" weight={iconWeight} />
			)}
			<span className="flex-1 truncate text-left">{label}</span>
			{rightComponent}
		</button>
	);
}
