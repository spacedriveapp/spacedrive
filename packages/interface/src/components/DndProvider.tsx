import type { CollisionDetection } from "@dnd-kit/core";
import {
  DndContext,
  DragOverlay,
  PointerSensor,
  pointerWithin,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { Clock, Folders, Heart, House } from "@phosphor-icons/react";
import type { File, SdPath } from "@sd/ts-client";
import { useSidebarStore } from "@sd/ts-client";
import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import {
  useLibraryMutation,
  useSpacedriveClient,
} from "../contexts/SpacedriveContext";
import { File as FileComponent } from "../routes/explorer/File";
import { useFileOperationDialog } from "./modals/FileOperationModal";
import { useSpaces } from "./SpacesSidebar/hooks/useSpaces";

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
 *    - Moves a file into a location/volume/folder
 *    - Shows a blue ring around the target
 *    - Data: { action, targetType, targetId, targetPath? }
 *    - targetType: "location" | "volume" | "folder"
 *    - targetPath: SdPath (for locations, directly usable)
 *
 * 3. type: "space" | "group"
 *    - Legacy: Drops on the space root or group area (no specific item)
 *    - Adds item to space/group
 *    - Data: { type, spaceId, groupId? }
 */
export function DndProvider({ children }: { children: React.ReactNode }) {
  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8, // Require 8px movement before activating drag
      },
    })
  );
  const addItem = useLibraryMutation("spaces.add_item");
  const reorderItems = useLibraryMutation("spaces.reorder_items");
  const reorderGroups = useLibraryMutation("spaces.reorder_groups");
  const openFileOperation = useFileOperationDialog();
  const [activeItem, setActiveItem] = useState<any>(null);
  const client = useSpacedriveClient();
  const queryClient = useQueryClient();
  const { currentSpaceId } = useSidebarStore();
  const { data: spacesData } = useSpaces();
  const spaces = spacesData?.spaces;

  // Custom collision detection: prefer -top zones over -bottom zones to avoid double lines
  const customCollision: CollisionDetection = (args) => {
    const collisions = pointerWithin(args);
    if (!collisions || collisions.length === 0) return collisions;

    // If we have multiple collisions, prefer -top over -bottom
    const hasTop = collisions.find((c) => String(c.id).endsWith("-top"));
    const hasMiddle = collisions.find((c) => String(c.id).endsWith("-middle"));

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

    // Handle sortable reordering (no drag data, just active/over IDs)
    if (active.id !== over.id && !active.data.current?.type) {
      console.log("[DnD] Sortable reorder:", {
        activeId: active.id,
        overId: over.id,
      });

      const libraryId = client.getCurrentLibraryId();
      const currentSpace =
        spaces?.find((s: any) => s.id === currentSpaceId) ?? spaces?.[0];

      if (!(currentSpace && libraryId)) return;

      const queryKey = [
        "query:spaces.get_layout",
        libraryId,
        { space_id: currentSpace.id },
      ];
      const layout = queryClient.getQueryData(queryKey) as any;

      if (!layout) return;

      // Check if we're reordering groups
      const groups = layout.groups?.map((g: any) => g.group) || [];
      const isGroupReorder = groups.some((g: any) => g.id === active.id);

      if (isGroupReorder) {
        console.log("[DnD] Reordering groups");

        const oldIndex = groups.findIndex((g: any) => g.id === active.id);
        const newIndex = groups.findIndex((g: any) => g.id === over.id);

        if (oldIndex !== -1 && newIndex !== -1 && oldIndex !== newIndex) {
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
              space_id: currentSpace.id,
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

      // Reordering space items
      if (layout?.space_items) {
        const items = layout.space_items;
        const oldIndex = items.findIndex((item: any) => item.id === active.id);

        // Extract item ID from over.id (could be a drop zone ID like "space-item-{id}-top")
        let overItemId = String(over.id);
        if (overItemId.startsWith("space-item-")) {
          // Extract the UUID from "space-item-{uuid}-top/bottom/middle"
          const parts = overItemId.split("-");
          // Remove "space" and "item" and the last part (top/bottom/middle)
          overItemId = parts.slice(2, -1).join("-");
        }

        const newIndex = items.findIndex((item: any) => item.id === overItemId);

        console.log("[DnD] Reorder space items:", {
          oldIndex,
          newIndex,
          activeId: active.id,
          extractedOverId: overItemId,
        });

        if (oldIndex !== -1 && newIndex !== -1 && oldIndex !== newIndex) {
          // Optimistically update the UI
          const newItems = [...items];
          const [movedItem] = newItems.splice(oldIndex, 1);
          newItems.splice(newIndex, 0, movedItem);

          queryClient.setQueryData(queryKey, {
            ...layout,
            space_items: newItems,
          });

          // Send reorder mutation
          try {
            await reorderItems.mutateAsync({
              group_id: null, // Space-level items
              item_ids: newItems.map((item: any) => item.id),
            });
            console.log("[DnD] Space items reorder successful");
          } catch (err) {
            console.error("[DnD] Space items reorder failed:", err);
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
      const currentSpace =
        spaces?.find((s: any) => s.id === currentSpaceId) ?? spaces?.[0];

      if (!(currentSpace && libraryId)) return;

      try {
        await addItem.mutateAsync({
          space_id: currentSpace.id,
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

    // Add to space (root-level drop zones between groups)
    if (dropData?.action === "add-to-space") {
      if (!dropData.spaceId) return;

      try {
        await addItem.mutateAsync({
          space_id: dropData.spaceId,
          group_id: null,
          item_type: { Path: { sd_path: dragData.sdPath } },
        });
        console.log("[DnD] Successfully added to space root");
      } catch (err) {
        console.error("[DnD] Failed to add to space:", err);
      }
      return;
    }

    // Add to group (empty group drop zone)
    if (dropData?.action === "add-to-group") {
      if (!(dropData.spaceId && dropData.groupId)) return;

      console.log("[DnD] Adding to group:", {
        spaceId: dropData.spaceId,
        groupId: dropData.groupId,
        sdPath: dragData.sdPath,
      });

      try {
        await addItem.mutateAsync({
          space_id: dropData.spaceId,
          group_id: dropData.groupId,
          item_type: { Path: { sd_path: dragData.sdPath } },
        });
        console.log("[DnD] Successfully added to group");
      } catch (err) {
        console.error("[DnD] Failed to add to group:", err);
      }
      return;
    }

    // Insert before/after sidebar items (adds item to space/group)
    if (
      dropData?.action === "insert-before" ||
      dropData?.action === "insert-after"
    ) {
      if (!dropData.spaceId) return;

      console.log("[DnD] Inserting item:", {
        action: dropData.action,
        spaceId: dropData.spaceId,
        groupId: dropData.groupId,
        sdPath: dragData.sdPath,
      });

      try {
        await addItem.mutateAsync({
          space_id: dropData.spaceId,
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

    // Move file into location/volume/folder
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

      // Determine operation based on modifier keys
      // For now default to copy (user can choose in modal)
      const operation = "copy";

      openFileOperation({
        operation,
        sources,
        destination,
      });
      return;
    }

    // Drop on space root area (adds to space)
    if (dropData?.type === "space" && dragData.type === "explorer-file") {
      console.log("[DnD] Adding to space (type=space):", {
        spaceId: dropData.spaceId,
        sdPath: dragData.sdPath,
      });

      try {
        await addItem.mutateAsync({
          space_id: dropData.spaceId,
          group_id: null,
          item_type: { Path: { sd_path: dragData.sdPath } },
        });
        console.log("[DnD] Successfully added to space");
      } catch (err) {
        console.error("[DnD] Failed to add item:", err);
      }
    }

    // Drop on group area (adds to group)
    if (dropData?.type === "group" && dragData.type === "explorer-file") {
      console.log("[DnD] Adding to group (type=group):", {
        spaceId: dropData.spaceId,
        groupId: dropData.groupId,
        sdPath: dragData.sdPath,
      });

      try {
        await addItem.mutateAsync({
          space_id: dropData.spaceId,
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
      collisionDetection={customCollision}
      onDragEnd={handleDragEnd}
      onDragStart={handleDragStart}
      sensors={sensors}
    >
      {children}
      <DragOverlay dropAnimation={null}>
        {activeItem?.type === "palette-item" ? (
          // Palette item preview
          <div className="flex min-w-[180px] items-center gap-2 rounded-lg bg-accent px-3 py-2 text-white shadow-lg">
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
            <span className="font-medium text-sm">
              {activeItem.itemType === "Overview" && "Overview"}
              {activeItem.itemType === "Recents" && "Recents"}
              {activeItem.itemType === "Favorites" && "Favorites"}
              {activeItem.itemType === "FileKinds" && "File Kinds"}
            </span>
          </div>
        ) : activeItem?.label ? (
          // Group or SpaceItem preview (from sortable context)
          <div className="flex min-w-[180px] items-center gap-2 rounded-lg border border-sidebar-line bg-sidebar/95 px-3 py-2 text-sidebar-ink shadow-lg backdrop-blur-sm">
            <span className="font-medium text-sm">{activeItem.label}</span>
          </div>
        ) : activeItem?.file ? (
          activeItem.gridSize ? (
            // Grid view preview
            <div style={{ width: activeItem.gridSize }}>
              <div className="relative flex flex-col items-center gap-2 rounded-lg p-1">
                <div className="rounded-lg p-2">
                  <FileComponent.Thumb
                    file={activeItem.file}
                    size={Math.max(activeItem.gridSize * 0.6, 60)}
                  />
                </div>
                <div className="max-w-full truncate rounded-md bg-accent px-2 py-0.5 text-sm text-white">
                  {activeItem.name}
                </div>
                {/* Show count badge if dragging multiple files */}
                {activeItem.selectedFiles &&
                  activeItem.selectedFiles.length > 1 && (
                    <div className="absolute -top-2 -right-2 flex size-6 items-center justify-center rounded-full border-2 border-app bg-accent font-bold text-white text-xs shadow-lg">
                      {activeItem.selectedFiles.length}
                    </div>
                  )}
              </div>
            </div>
          ) : (
            // Column/List view preview
            <div className="flex min-w-[200px] max-w-[300px] items-center gap-2 rounded-md bg-accent px-3 py-1.5 text-white shadow-lg">
              <FileComponent.Thumb file={activeItem.file} size={24} />
              <span className="truncate font-medium text-sm">
                {activeItem.name}
              </span>
              {/* Show count badge if dragging multiple files */}
              {activeItem.selectedFiles &&
                activeItem.selectedFiles.length > 1 && (
                  <div className="ml-auto flex size-5 items-center justify-center rounded-full bg-white font-bold text-accent text-xs">
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
