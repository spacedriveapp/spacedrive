import { useNavigate, useLocation } from "react-router-dom";
import clsx from "clsx";
import { useState, useEffect } from "react";
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
	Database,
	Folders,
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
import { useDroppable, useDndContext } from "@dnd-kit/core";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useExplorer } from "../Explorer/context";

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
	/** Optional custom label to override automatic label detection */
	customLabel?: string;
	/** Whether this is the last item in the list (for showing bottom insertion line) */
	isLastItem?: boolean;
	/** Whether this item supports insertion (reordering) - false for system groups */
	allowInsertion?: boolean;
	/** The space ID this item belongs to (for adding items on insertion) */
	spaceId?: string;
	/** The group ID this item belongs to (for adding items on insertion) */
	groupId?: string | null;
	/** Whether this item is sortable (can be reordered) */
	sortable?: boolean;
}

function getItemIcon(itemType: ItemType): any {
	if (itemType === "Overview") return { type: "component", icon: House };
	if (itemType === "Recents") return { type: "component", icon: Clock };
	if (itemType === "Favorites") return { type: "component", icon: Heart };
	if (itemType === "FileKinds") return { type: "component", icon: Folders };
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
	if (itemType === "FileKinds") return "File Kinds";
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
	resolvedFile?: File,
	itemSdPath?: any
): string | null {
	if (itemType === "Overview") return "/";
	if (itemType === "Recents") return "/recents";
	if (itemType === "Favorites") return "/favorites";
	if (itemType === "FileKinds") return "/file-kinds";
	if (typeof itemType === "object" && "Location" in itemType) {
		// Use explorer route with location's SD path (passed from item.sd_path)
		if (itemSdPath) {
			return `/explorer?path=${encodeURIComponent(JSON.stringify(itemSdPath))}`;
		}
		return null;
	}
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
		return null;
	}
	if (typeof itemType === "object" && "Tag" in itemType)
		return `/tag/${itemType.Tag.tag_id}`;
	if (typeof itemType === "object" && "Path" in itemType) {
		// Navigate to explorer with the SD path
		return `/explorer?path=${encodeURIComponent(JSON.stringify(itemType.Path.sd_path))}`;
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
	customLabel,
	isLastItem = false,
	allowInsertion = true,
	spaceId,
	groupId,
	sortable = false,
}: SpaceItemProps) {
	const navigate = useNavigate();
	const location = useLocation();
	const platform = usePlatform();
	const deleteItem = useLibraryMutation("spaces.delete_item");
	const indexVolume = useLibraryMutation("volumes.index");
	const { active } = useDndContext();
	const { currentView, currentPath } = useExplorer();
	
	// Disable insertion drop zones when dragging groups or space items (they have 'label' in their data)
	const isDraggingSortableItem = active?.data?.current?.label != null;

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
		// Use explorer path with the location's sd_path
		const sdPath = (item as any).sd_path;
		path = sdPath ? `/explorer?path=${encodeURIComponent(JSON.stringify(sdPath))}` : null;
	} else {
		// Handle proper SpaceItem
		iconData = getItemIcon(item.item_type);
		// Use resolved file name if available, otherwise parse from item_type
		label = resolvedFile?.name || getItemLabel(item.item_type);
		// Pass item.sd_path for locations (available on SpaceItem objects)
		path = getItemPath(item.item_type, volumeData, resolvedFile, (item as any).sd_path);
	}

	// Override with custom icon if provided
	if (customIcon) {
		iconData = { type: "image", icon: customIcon };
	}

	// Override with custom label if provided
	if (customLabel) {
		label = customLabel;
	}

	// Sortable hook (for reordering) - must be after label is defined
	const sortableProps = useSortable({
		id: item.id,
		disabled: !sortable,
		data: {
			label: label,
		},
	});

	const {
		attributes: sortableAttributes,
		listeners: sortableListeners,
		setNodeRef: setSortableRef,
		transform,
		transition,
		isDragging: isSortableDragging,
	} = sortableProps;

	const style = sortable ? {
		transform: CSS.Transform.toString(transform),
		transition,
	} : undefined;

	// Check if this item is active
	const isActive = (() => {
		// Check virtual view state from Explorer context
		if (currentView) {
			// If this item has a custom onClick (like devices), check if it matches the current view
			if (onClick && currentView.view === "device" && currentView.id === item.id) {
				return true;
			}
		}

		// Check path-based navigation
		if (currentPath && path && path.startsWith("/explorer?")) {
			const itemPathParam = new URLSearchParams(path.split("?")[1]).get("path");
			if (itemPathParam) {
				try {
					const itemSdPath = JSON.parse(decodeURIComponent(itemPathParam));
					return JSON.stringify(currentPath) === JSON.stringify(itemSdPath);
				} catch {
					// Fall through to URL-based comparison
				}
			}
		}

		if (!path) return false;

		// Special routes: exact pathname match
		if (!path.startsWith("/explorer?")) {
			return location.pathname === path;
		}

		// Fallback: Explorer routes via URL comparison
		if (location.pathname === "/explorer") {
			const currentSearchParams = new URLSearchParams(location.search);
			const currentPathParam = currentSearchParams.get("path");
			const itemPathParam = new URLSearchParams(path.split("?")[1]).get("path");

			if (currentPathParam && itemPathParam) {
				try {
					const currentSdPath = JSON.parse(decodeURIComponent(currentPathParam));
					const itemSdPath = JSON.parse(decodeURIComponent(itemPathParam));
					return JSON.stringify(currentSdPath) === JSON.stringify(itemSdPath);
				} catch {
					return currentPathParam === itemPathParam;
				}
			}
		}
		
		return false;
	})();

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
				icon: Database,
				label: "Index Volume",
				onClick: async () => {
					if (typeof item.item_type === "object" && "Volume" in item.item_type) {
						const volumeItem = item.item_type.Volume;
						// Extract volume fingerprint from the item
						// We'll need to get this from the volume data
						const fingerprint = (item as any).fingerprint || volumeItem.volume_id;

						try {
							const result = await indexVolume.mutateAsync({
								fingerprint: fingerprint.toString(),
								scope: "Recursive",
							});
							console.log("Volume indexed:", result.message);
						} catch (err) {
							console.error("Failed to index volume:", err);
						}
					}
				},
				condition: () => typeof item.item_type === "object" && "Volume" in item.item_type,
			},
			{ type: "separator" },
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
				try {
					await deleteItem.mutateAsync({ item_id: item.id });
				} catch (err) {
					console.error("Failed to remove item:", err);
				}
			},
			variant: "danger" as const,
			// All space items can be removed (Overview, Recents, Favorites, FileKinds, Locations, Volumes, Tags, Paths)
			condition: () => spaceId != null,
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

	// Debug logging for folder drop targets
	useEffect(() => {
		if (typeof item.item_type === "object" && "Path" in item.item_type) {
			console.log("[SpaceItem] Folder item:", {
				label,
				isDropTarget,
				targetType,
				hasResolvedFile: !!resolvedFile,
				resolvedFileKind: resolvedFile?.kind,
				sdPath: item.item_type.Path.sd_path,
			});
		}
	}, [item, isDropTarget, targetType, resolvedFile, label]);

	const { setNodeRef: setTopRef, isOver: isOverTop } = useDroppable({
		id: `space-item-${item.id}-top`,
		disabled: !allowInsertion || isDraggingSortableItem,
		data: {
			action: "insert-before",
			itemId: item.id,
			spaceId,
			groupId,
		},
	});

	const { setNodeRef: setBottomRef, isOver: isOverBottom } = useDroppable({
		id: `space-item-${item.id}-bottom`,
		disabled: !allowInsertion || isDraggingSortableItem,
		data: {
			action: "insert-after",
			itemId: item.id,
			spaceId,
			groupId,
		},
	});

	// Build the target path for drop operations
	const targetPath = isRawLocation
		? (item as any).sd_path
		: targetType === "folder" && typeof item.item_type === "object" && "Path" in item.item_type
		? item.item_type.Path.sd_path
		: targetType === "volume" && typeof item.item_type === "object" && "Volume" in item.item_type && volumeData
		? { Physical: { device_slug: volumeData.device_slug, path: volumeData.mount_path || "/" } }
		: targetType === "location" && typeof item.item_type === "object" && "Location" in item.item_type && (item as any).sd_path
		? (item as any).sd_path
		: undefined;

	// Debug log the drop data
	useEffect(() => {
		if (isDropTarget && targetType === "folder") {
			console.log("[SpaceItem] Drop zone data for folder:", {
				label,
				targetType,
				targetPath,
				itemId: item.id,
			});
		}
	}, [isDropTarget, targetType, targetPath, label, item.id]);

	const { setNodeRef: setMiddleRef, isOver: isOverMiddle } = useDroppable({
		id: `space-item-${item.id}-middle`,
		disabled: !isDropTarget || isDraggingSortableItem,
		data: {
			action: "move-into",
			targetType,
			targetId: item.id,
			targetPath,
		},
	});

	return (
		<div
			ref={setSortableRef}
			style={style}
			className={clsx("relative", isSortableDragging && "opacity-50 z-50")}
		>
			{/* Insertion line indicator - only show top (bottom of previous item handles gaps) */}
		{isOverTop && !isSortableDragging && !isDraggingSortableItem && (
			<div className="absolute -top-[1px] left-2 right-2 h-[2px] bg-accent z-20 rounded-full" />
		)}

			{/* Ring highlight for drop-into */}
		{isOverMiddle && isDropTarget && !isSortableDragging && !isDraggingSortableItem && (
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
					{...(sortable ? { ...sortableAttributes, ...sortableListeners } : {})}
				className={clsx(
					"flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm font-medium transition-colors relative cursor-default",
					className ||
						(isActive
							? "bg-sidebar-selected/30 text-sidebar-ink"
							: "text-sidebar-inkDull"),
					isOverMiddle && isDropTarget && !isDraggingSortableItem && "bg-accent/10",
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
		{isOverBottom && isLastItem && !isDraggingSortableItem && (
			<div className="absolute -bottom-[1px] left-2 right-2 h-[2px] bg-accent z-20 rounded-full" />
		)}
		</div>
	);
}
