import {
	DndContext,
	DragOverlay,
	PointerSensor,
	useSensor,
	useSensors,
	pointerWithin,
} from "@dnd-kit/core";
import type { CollisionDetection } from "@dnd-kit/core";
import { useState } from "react";
import { House, Clock, Heart, Folders } from "@phosphor-icons/react";
import { useQueryClient } from "@tanstack/react-query";
import { useLibraryMutation, useSpacedriveClient } from "../contexts/SpacedriveContext";
import { useSidebarStore } from "@sd/ts-client";
import type { File, SdPath } from "@sd/ts-client";
import { useLayouts } from "./Sidebar/hooks/useLayouts";
import { useFileOperationDialog } from "./modals/FileOperationModal";
import { useMergeFoldersDialog } from "./modals/MergeFoldersModal";
import { File as FileComponent } from "../routes/explorer/File";
import { useTabManager } from "./TabManager/useTabManager";
import { modifierKeysNow, useModifierKeyTracker } from "../hooks/useModifierKeys";

/**
 * DndProvider - Global drag-and-drop coordinator
 *
 * Handles all drag-and-drop operations in the Explorer using @dnd-kit/core.
 *
 * Drop Actions:
 *
 * 1. insert-before / insert-after
 *    - Pins a file to the sidebar before/after an existing item
 *    - Shows a blue line indicator
 *    - Data: { action, itemId }
 *
 * 2. move-into
 *    - Moves a file into a volume/folder
 *    - Shows a blue ring around the target
 *    - Data: { action, targetType, targetId, targetPath? }
 *    - targetType: "volume" | "folder"
 *    - targetPath: SdPath, directly usable
 *    - Folders dropped on a folder with Option held open the merge dialog;
 *      dnd-kit carries no modifier state, so the window tracker answers
 *
 * 3. type: "layout" | "group"
 *    - Legacy: Drops on the layout root or group area (no specific item)
 *    - Adds item to layout/group
 *    - Data: { type, layoutId, groupId? }
 */
export function DndProvider({ children }: { children: React.ReactNode }) {
	const sensors = useSensors(
		useSensor(PointerSensor, {
			activationConstraint: {
				distance: 8, // Require 8px movement before activating drag
			},
		}),
	);
	const addItem = useLibraryMutation("layouts.add_item");
	const reorderItems = useLibraryMutation("layouts.reorder_items");
	const reorderGroups = useLibraryMutation("layouts.reorder_groups");
	const openFileOperation = useFileOperationDialog();
	const openMergeFolders = useMergeFoldersDialog();
	useModifierKeyTracker();
	const [activeItem, setActiveItem] = useState<any>(null);
	const client = useSpacedriveClient();
	const queryClient = useQueryClient();
	const { currentLayoutId } = useSidebarStore();
	const { data: layoutsData } = useLayouts();
	const layouts = layoutsData?.layouts;
	const { reorderTabs } = useTabManager();

	// Custom collision detection: prefer -top zones over -bottom zones to avoid double lines
	const customCollision: CollisionDetection = (args) => {
		const collisions = pointerWithin(args);
		if (!collisions || collisions.length === 0) return collisions;

		// If we have multiple collisions, prefer -top over -bottom
		const hasTop = collisions.find((c) => String(c.id).endsWith("-top"));
		const hasMiddle = collisions.find((c) =>
			String(c.id).endsWith("-middle"),
		);

		if (hasMiddle) return [hasMiddle]; // Middle zone takes priority
		if (hasTop) return [hasTop]; // Top zone over bottom
		return [collisions[0]]; // First collision
	};

	const handleDragStart = (event: any) => {
		setActiveItem(event.active.data.current);
	};

	const handleDragEnd = async (event: any) => {
		const { active, over } = event;

		setActiveItem(null);

		if (!over) return;

		// Handle tab reordering
		if (
			active.id !== over.id &&
			active.data.current?.type === "tab" &&
			over.data.current?.type === "tab"
		) {
			console.log("[DnD] Tab reorder:", {
				activeId: active.id,
				overId: over.id,
			});

			reorderTabs(active.id, over.id);
			return;
		}

		// Handle sortable reordering (no drag data, just active/over IDs)
		if (active.id !== over.id && !active.data.current?.type) {
			console.log("[DnD] Sortable reorder:", {
				activeId: active.id,
				overId: over.id,
			});

			const libraryId = client.getCurrentLibraryId();
			const currentLayout =
				layouts?.find((s: any) => s.id === currentLayoutId) ??
				layouts?.[0];

			if (!currentLayout || !libraryId) return;

			const queryKey = [
				"query:layouts.get_sidebar",
				libraryId,
				{ layout_id: currentLayout.id },
			];
			const layout = queryClient.getQueryData(queryKey) as any;

			if (!layout) return;

			// Check if we're reordering groups
			const groups = layout.groups?.map((g: any) => g.group) || [];
			const isGroupReorder = groups.some((g: any) => g.id === active.id);

			if (isGroupReorder) {
				console.log("[DnD] Reordering groups");

				const oldIndex = groups.findIndex(
					(g: any) => g.id === active.id,
				);
				const newIndex = groups.findIndex((g: any) => g.id === over.id);

				if (
					oldIndex !== -1 &&
					newIndex !== -1 &&
					oldIndex !== newIndex
				) {
					// Optimistically update the UI
					const newGroups = [...layout.groups];
					const [movedGroup] = newGroups.splice(oldIndex, 1);
					newGroups.splice(newIndex, 0, movedGroup);

					queryClient.setQueryData(queryKey, {
						...layout,
						groups: newGroups,
					});

					// Send reorder mutation
					try {
						await reorderGroups.mutateAsync({
							layout_id: currentLayout.id,
							group_ids: newGroups.map((g: any) => g.group.id),
						});
						console.log("[DnD] Group reorder successful");
					} catch (err) {
						console.error("[DnD] Group reorder failed:", err);
						// Revert on error
						queryClient.setQueryData(queryKey, layout);
					}
				}

				return;
			}

			// Reordering layout items
			if (layout?.layout_items) {
				const items = layout.layout_items;
				const oldIndex = items.findIndex(
					(item: any) => item.id === active.id,
				);

				// Extract item ID from over.id (could be a drop zone ID like "layout-item-{id}-top")
				let overItemId = String(over.id);
				if (overItemId.startsWith("layout-item-")) {
					// Extract the UUID from "layout-item-{uuid}-top/bottom/middle"
					const parts = overItemId.split("-");
					// Remove "layout" and "item" and the last part (top/bottom/middle)
					overItemId = parts.slice(2, -1).join("-");
				}

				const newIndex = items.findIndex(
					(item: any) => item.id === overItemId,
				);

				console.log("[DnD] Reorder layout items:", {
					oldIndex,
					newIndex,
					activeId: active.id,
					extractedOverId: overItemId,
				});

				if (
					oldIndex !== -1 &&
					newIndex !== -1 &&
					oldIndex !== newIndex
				) {
					// Optimistically update the UI
					const newItems = [...items];
					const [movedItem] = newItems.splice(oldIndex, 1);
					newItems.splice(newIndex, 0, movedItem);

					queryClient.setQueryData(queryKey, {
						...layout,
						layout_items: newItems,
					});

					// Send reorder mutation
					try {
						await reorderItems.mutateAsync({
							group_id: null, // Layout-level items
							item_ids: newItems.map((item: any) => item.id),
						});
						console.log("[DnD] Layout items reorder successful");
					} catch (err) {
						console.error("[DnD] Layout items reorder failed:", err);
						// Revert on error
						queryClient.setQueryData(queryKey, layout);
					}
				}
			}

			return;
		}

		if (!active.data.current) return;

		const dragData = active.data.current;
		const dropData = over.data.current;

		// Handle palette item drops (from customization panel)
		if (dragData?.type === "palette-item") {
			const libraryId = client.getCurrentLibraryId();
			const currentLayout =
				layouts?.find((s: any) => s.id === currentLayoutId) ??
				layouts?.[0];

			if (!currentLayout || !libraryId) return;

			try {
				await addItem.mutateAsync({
					layout_id: currentLayout.id,
					group_id: dropData?.groupId || null,
					item_type: dragData.itemType,
				});
				console.log("[DnD] Successfully added palette item");
			} catch (err) {
				console.error("[DnD] Failed to add palette item:", err);
			}
			return;
		}

		if (!dragData || dragData.type !== "explorer-file") return;

		// Add to layout (root-level drop zones between groups)
		if (dropData?.action === "add-to-layout") {
			if (!dropData.layoutId) return;

			try {
				await addItem.mutateAsync({
					layout_id: dropData.layoutId,
					group_id: null,
					item_type: { Path: { sd_path: dragData.sdPath } },
				});
				console.log("[DnD] Successfully added to layout root");
			} catch (err) {
				console.error("[DnD] Failed to add to layout:", err);
			}
			return;
		}

		// Add to group (empty group drop zone)
		if (dropData?.action === "add-to-group") {
			if (!dropData.layoutId || !dropData.groupId) return;

			console.log("[DnD] Adding to group:", {
				layoutId: dropData.layoutId,
				groupId: dropData.groupId,
				sdPath: dragData.sdPath,
			});

			try {
				await addItem.mutateAsync({
					layout_id: dropData.layoutId,
					group_id: dropData.groupId,
					item_type: { Path: { sd_path: dragData.sdPath } },
				});
				console.log("[DnD] Successfully added to group");
			} catch (err) {
				console.error("[DnD] Failed to add to group:", err);
			}
			return;
		}

		// Insert before/after sidebar items (adds item to layout/group)
		if (
			dropData?.action === "insert-before" ||
			dropData?.action === "insert-after"
		) {
			if (!dropData.layoutId) return;

			console.log("[DnD] Inserting item:", {
				action: dropData.action,
				layoutId: dropData.layoutId,
				groupId: dropData.groupId,
				sdPath: dragData.sdPath,
			});

			try {
				await addItem.mutateAsync({
					layout_id: dropData.layoutId,
					group_id: dropData.groupId || null,
					item_type: { Path: { sd_path: dragData.sdPath } },
				});
				console.log("[DnD] Successfully inserted item");
				// TODO: Implement proper ordering relative to itemId
			} catch (err) {
				console.error("[DnD] Failed to add item:", err);
			}
			return;
		}

		// Move file into volume/folder
		if (dropData?.action === "move-into") {
			console.log("[DnD] Move-into action:", {
				targetType: dropData.targetType,
				targetId: dropData.targetId,
				targetPath: dropData.targetPath,
				hasTargetPath: !!dropData.targetPath,
				draggedFile: dragData.name,
			});

			const sources: SdPath[] = dragData.selectedFiles
				? dragData.selectedFiles.map((f: File) => f.sd_path)
				: [dragData.sdPath];

			const destination: SdPath = dropData.targetPath;

			if (!destination) {
				console.error("[DnD] No target path for move-into action");
				return;
			}

			// Folders dropped on a folder with Option held merge into it.
			// Without the key the operation modal decides, and offers a
			// merge itself when the drop collides with a same-name folder.
			const dragged: File[] = dragData.selectedFiles ?? [dragData.file];
			const folders =
				dragged.length > 0 && dragged.every((f) => f?.kind === "Directory");
			if (modifierKeysNow().alt && folders && dropData.targetType === "folder") {
				openMergeFolders({ sources, destination });
				return;
			}

			openFileOperation({
				operation: "copy",
				sources,
				destination,
			});
			return;
		}

		// Drop on layout root area (adds to layout)
		if (dropData?.type === "layout" && dragData.type === "explorer-file") {
			console.log("[DnD] Adding to layout (type=layout):", {
				layoutId: dropData.layoutId,
				sdPath: dragData.sdPath,
			});

			try {
				await addItem.mutateAsync({
					layout_id: dropData.layoutId,
					group_id: null,
					item_type: { Path: { sd_path: dragData.sdPath } },
				});
				console.log("[DnD] Successfully added to layout");
			} catch (err) {
				console.error("[DnD] Failed to add item:", err);
			}
		}

		// Drop on group area (adds to group)
		if (dropData?.type === "group" && dragData.type === "explorer-file") {
			console.log("[DnD] Adding to group (type=group):", {
				layoutId: dropData.layoutId,
				groupId: dropData.groupId,
				sdPath: dragData.sdPath,
			});

			try {
				await addItem.mutateAsync({
					layout_id: dropData.layoutId,
					group_id: dropData.groupId,
					item_type: { Path: { sd_path: dragData.sdPath } },
				});
				console.log("[DnD] Successfully added to group");
			} catch (err) {
				console.error("[DnD] Failed to add item to group:", err);
			}
		}
	};

	return (
		<DndContext
			sensors={sensors}
			collisionDetection={customCollision}
			onDragStart={handleDragStart}
			onDragEnd={handleDragEnd}
		>
			{children}
			<DragOverlay dropAnimation={null}>
				{activeItem?.type === "palette-item" ? (
					// Palette item preview
					<div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-accent text-white shadow-lg min-w-[180px]">
						{activeItem.itemType === "Overview" && (
							<House size={20} weight="bold" />
						)}
						{activeItem.itemType === "Recents" && (
							<Clock size={20} weight="bold" />
						)}
						{activeItem.itemType === "Favorites" && (
							<Heart size={20} weight="bold" />
						)}
						{activeItem.itemType === "FileKinds" && (
							<Folders size={20} weight="bold" />
						)}
						<span className="text-sm font-medium">
							{activeItem.itemType === "Overview" && "Overview"}
							{activeItem.itemType === "Recents" && "Recents"}
							{activeItem.itemType === "Favorites" && "Favorites"}
							{activeItem.itemType === "FileKinds" &&
								"File Kinds"}
						</span>
					</div>
				) : activeItem?.label ? (
					// Group or LayoutItem preview (from sortable context)
					<div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-sidebar/95 backdrop-blur-sm text-sidebar-ink shadow-lg border border-sidebar-line min-w-[180px]">
						<span className="text-sm font-medium">
							{activeItem.label}
						</span>
					</div>
				) : activeItem?.file ? (
					activeItem.gridSize ? (
						// Grid view preview
						<div style={{ width: activeItem.gridSize }}>
							<div className="flex flex-col items-center gap-2 p-1 rounded-lg relative">
								<div className="rounded-lg p-2">
									<FileComponent.Thumb
										file={activeItem.file}
										size={Math.max(
											activeItem.gridSize * 0.6,
											60,
										)}
									/>
								</div>
								<div className="text-sm truncate px-2 py-0.5 rounded-md bg-accent text-white max-w-full">
									{activeItem.name}
								</div>
								{/* Show count badge if dragging multiple files */}
								{activeItem.selectedFiles &&
									activeItem.selectedFiles.length > 1 && (
										<div className="absolute -top-2 -right-2 size-6 rounded-full bg-accent text-white text-xs font-bold flex items-center justify-center shadow-lg border-2 border-app">
											{activeItem.selectedFiles.length}
										</div>
									)}
							</div>
						</div>
					) : (
						// Column/List view preview
						<div className="flex items-center gap-2 px-3 py-1.5 rounded-md bg-accent text-white shadow-lg min-w-[200px] max-w-[300px]">
							<FileComponent.Thumb
								file={activeItem.file}
								size={24}
							/>
							<span className="text-sm font-medium truncate">
								{activeItem.name}
							</span>
							{/* Show count badge if dragging multiple files */}
							{activeItem.selectedFiles &&
								activeItem.selectedFiles.length > 1 && (
									<div className="ml-auto size-5 rounded-full bg-white text-accent text-xs font-bold flex items-center justify-center">
										{activeItem.selectedFiles.length}
									</div>
								)}
						</div>
					)
				) : null}
			</DragOverlay>
		</DndContext>
	);
}