import { useNavigate } from "react-router-dom";
import {
	FolderOpen,
	MagnifyingGlass,
	Trash,
	Database,
} from "@phosphor-icons/react";
import type { SpaceItem as SpaceItemType } from "@sd/ts-client";
import {
	useContextMenu,
	type ContextMenuItem,
	type ContextMenuResult,
} from "../../../hooks/useContextMenu";
import { usePlatform } from "../../../platform";
import { useLibraryMutation } from "../../../context";
import { isVolumeItem, isPathItem } from "./spaceItemUtils";

interface UseSpaceItemContextMenuOptions {
	item: SpaceItemType;
	path: string | null;
	spaceId?: string;
}

/**
 * Provides context menu functionality for space items.
 *
 * Menu items include:
 * - Open: Navigate to the item's path
 * - Index Volume: Trigger indexing for volume items
 * - Show in Finder: Reveal file in OS file manager (Path items only)
 * - Remove from Space: Delete the item from the current space
 */
export function useSpaceItemContextMenu({
	item,
	path,
	spaceId,
}: UseSpaceItemContextMenuOptions): ContextMenuResult {
	const navigate = useNavigate();
	const platform = usePlatform();
	const deleteItem = useLibraryMutation("spaces.delete_item");
	const indexVolume = useLibraryMutation("volumes.index");

	const items: ContextMenuItem[] = [
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
				if (isVolumeItem(item.item_type)) {
					const fingerprint =
						(item as SpaceItemType & { fingerprint?: string })
							.fingerprint || item.item_type.Volume.volume_id;

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
			condition: () => isVolumeItem(item.item_type),
		},
		{ type: "separator" },
		{
			icon: MagnifyingGlass,
			label: "Show in Finder",
			onClick: async () => {
				if (isPathItem(item.item_type)) {
					const sdPath = item.item_type.Path.sd_path;
					if (typeof sdPath === "object" && "Physical" in sdPath) {
						const physicalPath = (
							sdPath as { Physical: { path: string } }
						).Physical.path;
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
				if (!isPathItem(item.item_type)) return false;
				const sdPath = item.item_type.Path.sd_path;
				return (
					typeof sdPath === "object" &&
					"Physical" in sdPath &&
					!!platform.revealFile
				);
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
			condition: () => spaceId != null,
		},
	];

	return useContextMenu({ items });
}
