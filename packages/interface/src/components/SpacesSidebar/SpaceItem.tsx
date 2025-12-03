import { useNavigate, useLocation } from "react-router-dom";
import clsx from "clsx";
import { useState } from "react";
import {
	House,
	Clock,
	Heart,
	Folder,
	HardDrive,
	Tag as TagIcon,
	FolderOpen,
	MagnifyingGlass,
	Trash,
} from "@phosphor-icons/react";
import { Location } from "@sd/assets/icons";
import type {
	SpaceItem as SpaceItemType,
	ItemType,
	File,
} from "@sd/ts-client";
import { Thumb } from "../Explorer/File/Thumb";
import { useContextMenu } from "../../hooks/useContextMenu";
import { usePlatform } from "../../platform";
import { useLibraryMutation } from "../../context";
import { useDroppable } from "@dnd-kit/core";

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
	/** Whether this is the last item in the list (for showing bottom insertion line) */
	isLastItem?: boolean;
	/** Whether this item supports insertion (reordering) - false for system groups */
	allowInsertion?: boolean;
	/** The space ID this item belongs to (for adding items on insertion) */
	spaceId?: string;
	/** The group ID this item belongs to (for adding items on insertion) */
	groupId?: string | null;
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

function getItemPath(
	itemType: ItemType,
	volumeData?: { device_slug: string; mount_path: string },
	resolvedFile?: File
): string | null {
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
	if (typeof itemType === "object" && "Path" in itemType) {
		// If it's a directory, navigate to explorer
		if (resolvedFile?.kind === "Directory") {
			return `/explorer?path=${encodeURIComponent(JSON.stringify(itemType.Path.sd_path))}`;
		}
		// Regular files don't have a path to navigate to (could open/preview in future)
		return null;
	}
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
	isLastItem = false,
	allowInsertion = true,
	spaceId,
	groupId,
}: SpaceItemProps) {
	const navigate = useNavigate();
	const location = useLocation();
	const platform = usePlatform();
	const deleteItem = useLibraryMutation("spaces.delete_item");

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
		path = getItemPath(item.item_type, volumeData, resolvedFile);
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

	// Context menu for space items
	const contextMenu = useContextMenu({
		items: [
			{
				icon: FolderOpen,
				label: "Open",
				onClick: () => {
					if (path) navigate(path);
				},
				condition: () => !!path,
			},
			{
				icon: MagnifyingGlass,
				label: "Show in Finder",
				onClick: async () => {
					// For Path items, get the physical path
					if (typeof item.item_type === "object" && "Path" in item.item_type) {
						const sdPath = item.item_type.Path.sd_path;
						if (typeof sdPath === "object" && "Physical" in sdPath) {
							const physicalPath = sdPath.Physical.path;
							if (platform.revealFile) {
								try {
									await platform.revealFile(physicalPath);
								} catch (err) {
									console.error("Failed to reveal file:", err);
								}
							}
						}
					}
				},
				keybind: "⌘⇧R",
				condition: () => {
					if (typeof item.item_type === "object" && "Path" in item.item_type) {
						const sdPath = item.item_type.Path.sd_path;
						return typeof sdPath === "object" && "Physical" in sdPath && !!platform.revealFile;
					}
					return false;
				},
			},
			{ type: "separator" },
			{
				icon: Trash,
				label: "Remove from Space",
				onClick: async () => {
					if (confirm(`Remove "${label}" from this space?`)) {
						try {
							await deleteItem.mutateAsync({ item_id: item.id });
						} catch (err) {
							console.error("Failed to remove item:", err);
						}
					}
				},
				variant: "danger" as const,
				// Can only remove custom Path items, not built-in items
				condition: () => typeof item.item_type === "object" && "Path" in item.item_type,
			},
		],
	});

	const handleContextMenu = async (e: React.MouseEvent) => {
		e.preventDefault();
		e.stopPropagation();
		await contextMenu.show(e);
	};

	/**
	 * Drop Target Detection
	 *
	 * SpaceItems can be drop targets in two ways:
	 *
	 * 1. Insertion Points (all items):
	 *    - Show blue line above/below
	 *    - Allows reordering sidebar items
	 *    - Top/bottom zones (25% or 50% of height)
	 *
	 * 2. Move-Into Targets (locations/volumes/folders only):
	 *    - Show blue ring around entire item
	 *    - Allows moving files into that location
	 *    - Middle zone (50% of height, only for drop targets)
	 *
	 * Target Types:
	 * - "location": Indexed location (raw or ItemType::Location)
	 * - "volume": Storage volume (ItemType::Volume)
	 * - "folder": Directory path (ItemType::Path with kind=Directory)
	 */
	const isDropTarget =
		isRawLocation ||
		(typeof item.item_type === "object" &&
		 ("Location" in item.item_type ||
		  "Volume" in item.item_type ||
		  ("Path" in item.item_type && resolvedFile?.kind === "Directory")));

	let targetType: "location" | "volume" | "folder" | "other" = "other";
	if (isRawLocation) {
		targetType = "location";
	} else if (typeof item.item_type === "object") {
		if ("Location" in item.item_type) targetType = "location";
		else if ("Volume" in item.item_type) targetType = "volume";
		else if ("Path" in item.item_type && resolvedFile?.kind === "Directory") targetType = "folder";
	}

	const { setNodeRef: setTopRef, isOver: isOverTop } = useDroppable({
		id: `space-item-${item.id}-top`,
		disabled: !allowInsertion,
		data: {
			action: "insert-before",
			itemId: item.id,
			spaceId,
			groupId,
		},
	});

	const { setNodeRef: setBottomRef, isOver: isOverBottom } = useDroppable({
		id: `space-item-${item.id}-bottom`,
		disabled: !allowInsertion,
		data: {
			action: "insert-after",
			itemId: item.id,
			spaceId,
			groupId,
		},
	});

	const { setNodeRef: setMiddleRef, isOver: isOverMiddle } = useDroppable({
		id: `space-item-${item.id}-middle`,
		disabled: !isDropTarget,
		data: {
			action: "move-into",
			targetType,
			targetId: item.id,
			// For raw locations, include the sd_path directly
			targetPath: isRawLocation ? (item as any).sd_path : undefined,
		},
	});

	return (
		<div className="relative">
			{/* Insertion line indicator - only show top (bottom of previous item handles gaps) */}
			{isOverTop && (
				<div className="absolute -top-[1px] left-2 right-2 h-[2px] bg-accent z-20 rounded-full" />
			)}

			{/* Ring highlight for drop-into */}
			{isOverMiddle && isDropTarget && (
				<div className="absolute inset-0 rounded-md ring-2 ring-accent/50 ring-inset pointer-events-none z-10" />
			)}

			<div className="relative">
				{/* Drop zones - invisible overlays, only active during drag */}
				{isDropTarget ? (
					<>
						{/* Top zone - insertion above */}
						<div
							ref={setTopRef}
							className="absolute left-0 right-0 pointer-events-none"
							style={{ top: "-2px", height: "calc(25% + 2px)", zIndex: 10 }}
						/>
						{/* Middle zone - drop into folder */}
						<div
							ref={setMiddleRef}
							className="absolute left-0 right-0 pointer-events-none"
							style={{ top: "25%", height: "50%", zIndex: 11 }}
						/>
						{/* Bottom zone - insertion below */}
						<div
							ref={setBottomRef}
							className="absolute left-0 right-0 pointer-events-none"
							style={{ bottom: "-2px", height: "calc(25% + 2px)", zIndex: 10 }}
						/>
					</>
				) : (
					<>
						{/* Top zone - insertion above */}
						<div
							ref={setTopRef}
							className="absolute left-0 right-0 pointer-events-none"
							style={{ top: "-2px", height: "calc(50% + 2px)", zIndex: 10 }}
						/>
						{/* Bottom zone - insertion below */}
						<div
							ref={setBottomRef}
							className="absolute left-0 right-0 pointer-events-none"
							style={{ bottom: "-2px", height: "calc(50% + 2px)", zIndex: 10 }}
						/>
					</>
				)}

				<button
					onClick={handleClick}
					onContextMenu={handleContextMenu}
					className={clsx(
						"flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm font-medium transition-colors relative cursor-default",
						className ||
							(isActive
								? "bg-sidebar-selected/30 text-sidebar-ink"
								: "text-sidebar-inkDull"),
						isOverMiddle && isDropTarget && "bg-accent/10",
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
			</div>

			{/* Insertion line indicator - bottom (only for last item to allow dropping at end) */}
			{isOverBottom && isLastItem && (
				<div className="absolute -bottom-[1px] left-2 right-2 h-[2px] bg-accent z-20 rounded-full" />
			)}
		</div>
	);
}
