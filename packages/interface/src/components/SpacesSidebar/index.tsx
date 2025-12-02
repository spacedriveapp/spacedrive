import { useState, useEffect, useRef } from "react";
import { GearSix } from "@phosphor-icons/react";
import { useNavigate } from "react-router-dom";
import { useSidebarStore, useLibraryMutation } from "@sd/ts-client";
import { useSpaces, useSpaceLayout } from "./hooks/useSpaces";
import { SpaceSwitcher } from "./SpaceSwitcher";
import { SpaceGroup } from "./SpaceGroup";
import { SpaceItem } from "./SpaceItem";
import { AddGroupButton } from "./AddGroupButton";
import { useSpacedriveClient } from "../../context";
import { useLibraries } from "../../hooks/useLibraries";
import { usePlatform } from "../../platform";
import { JobManagerPopover } from "../JobManager/JobManagerPopover";
import { SyncMonitorPopover } from "../SyncMonitor";
import { getDragData, clearDragData, subscribeToDragState } from "./dnd";
import clsx from "clsx";

interface SpacesSidebarProps {
  isPreviewActive?: boolean;
}

export function SpacesSidebar({ isPreviewActive = false }: SpacesSidebarProps) {
  const client = useSpacedriveClient();
  const platform = usePlatform();
  const { data: libraries } = useLibraries();
  const navigate = useNavigate();
  const [currentLibraryId, setCurrentLibraryId] = useState<string | null>(
    () => client.getCurrentLibraryId(),
  );

  const { currentSpaceId, setCurrentSpace } = useSidebarStore();
  const { data: spacesData } = useSpaces();
  const spaces = spacesData?.spaces;

  // Listen for library changes from client and update local state
  useEffect(() => {
    const handleLibraryChange = (newLibraryId: string) => {
      setCurrentLibraryId(newLibraryId);
    };

    client.on("library-changed", handleLibraryChange);
    return () => {
      client.off("library-changed", handleLibraryChange);
    };
  }, [client]);

  // Auto-select first library on mount if none selected
  useEffect(() => {
    if (libraries && libraries.length > 0 && !currentLibraryId) {
      const firstLib = libraries[0];

      // Set library ID via platform (syncs to all windows on Tauri)
      if (platform.setCurrentLibraryId) {
        platform.setCurrentLibraryId(firstLib.id).catch((err) =>
          console.error("Failed to set library ID:", err),
        );
      } else {
        // Web fallback - just update client
        client.setCurrentLibrary(firstLib.id);
      }
    }
  }, [libraries, currentLibraryId, client, platform]);

  // Auto-select first space if none selected
  const currentSpace =
    spaces?.find((s) => s.id === currentSpaceId) ?? spaces?.[0];

  useEffect(() => {
    if (currentSpace && currentSpace.id !== currentSpaceId) {
      setCurrentSpace(currentSpace.id);
    }
  }, [currentSpace, currentSpaceId, setCurrentSpace]);

  const { data: layout } = useSpaceLayout(currentSpace?.id ?? null);

  // Drag-drop state
  const [isDragging, setIsDragging] = useState(false);
  const [isHovering, setIsHovering] = useState(false);
  const addItem = useLibraryMutation("spaces.add_item");
  const dropZoneRef = useRef<HTMLDivElement>(null);

  // Subscribe to drag state changes (from setDragData)
  useEffect(() => {
    return subscribeToDragState(setIsDragging);
  }, []);

  // Listen for native drag events to track position and handle drop
  useEffect(() => {
    if (!platform.onDragEvent) return;

    const unlisteners: Array<() => void> = [];

    // Track drag position to detect when over sidebar
    platform.onDragEvent("moved", (payload: { x: number; y: number }) => {
      if (!dropZoneRef.current) return;

      const rect = dropZoneRef.current.getBoundingClientRect();
      const isOver = (
        payload.x >= rect.left &&
        payload.x <= rect.right &&
        payload.y >= rect.top &&
        payload.y <= rect.bottom
      );
      setIsHovering(isOver);
    }).then(fn => unlisteners.push(fn));

    // Handle drag end - check if dropped on sidebar
    platform.onDragEvent("ended", async (payload: { result?: { type: string } }) => {
      const dragData = getDragData(); // Get BEFORE clearing

      // Check for "dropped" (lowercase from backend)
      const wasDropped = payload.result?.type?.toLowerCase() === "dropped";

      // If dropped and we have drag data from our app, add it to the space
      if (wasDropped && currentSpace && dragData) {
        try {
          await addItem.mutateAsync({
            space_id: currentSpace.id,
            group_id: null,
            item_type: { Path: { sd_path: dragData.sdPath } },
          });
        } catch (err) {
          console.error("Failed to add item to space:", err);
        }
      }

      clearDragData();
      setIsDragging(false);
      setIsHovering(false);
    }).then(fn => unlisteners.push(fn));

    return () => {
      unlisteners.forEach(fn => fn());
    };
  }, [platform, currentSpace, addItem, isHovering]);

  return (
    <div className="w-[220px] min-w-[176px] max-w-[300px] flex flex-col h-full p-2 bg-transparent">
      <div
        className={clsx(
          "flex flex-col h-full rounded-2xl overflow-hidden",
          isPreviewActive ? "backdrop-blur-2xl bg-sidebar/80" : "bg-sidebar/65",
        )}
      >
        <nav className="relative z-[51] flex h-full flex-col gap-2.5 p-2.5 pb-2 pt-[52px]">
          {/* Space Switcher */}
          <SpaceSwitcher
            spaces={spaces}
            currentSpace={currentSpace}
            onSwitch={setCurrentSpace}
          />

          {/* Scrollable Content - Drop Zone */}
          <div
            ref={dropZoneRef}
            className={clsx(
              "no-scrollbar mt-3 mask-fade-out flex grow flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10 transition-colors rounded-lg",
              isDragging && "bg-accent/10 ring-2 ring-accent/50 ring-inset",
              isDragging && isHovering && "bg-accent/20 ring-accent"
            )}
          >
            {/* Space-level items (pinned shortcuts) */}
            {layout?.space_items && layout.space_items.length > 0 && (
              <div className="space-y-0.5">
                {layout.space_items.map((item) => (
                  <SpaceItem key={item.id} item={item} />
                ))}
              </div>
            )}

            {/* Drop hint when dragging */}
            {isDragging && (
              <div className={clsx(
                "flex items-center justify-center py-4 text-xs font-medium transition-colors",
                isHovering ? "text-accent" : "text-accent/70"
              )}>
                {isHovering ? "Release to add shortcut" : "Drop to add shortcut"}
              </div>
            )}

            {/* Groups */}
            {layout?.groups.map(({ group, items }) => (
              <SpaceGroup key={group.id} group={group} items={items} spaceId={currentSpace?.id} />
            ))}

            {/* Add Group Button */}
            {currentSpace && <AddGroupButton spaceId={currentSpace.id} />}
          </div>

          {/* Sync Monitor, Job Manager & Settings (pinned to bottom) */}
          <div className="space-y-0.5">
            <SyncMonitorPopover />
            <JobManagerPopover />
            <button
              onClick={() => navigate("/settings")}
              className={clsx(
                "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm font-medium transition-colors",
                "text-sidebar-inkDull hover:text-sidebar-ink hover:bg-sidebar-selected",
              )}
            >
              <GearSix className="size-4" weight="bold" />
              <span className="truncate">Settings</span>
            </button>
          </div>
        </nav>
      </div>
    </div>
  );
}
