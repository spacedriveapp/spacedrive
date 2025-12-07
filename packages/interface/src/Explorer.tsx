import { SpacedriveProvider, type SpacedriveClient } from "./context";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import {
  RouterProvider,
  Outlet,
  useLocation,
  useParams,
} from "react-router-dom";
import { useEffect, useMemo } from "react";
import { Dialogs } from "@sd/ui";
import { Inspector, type InspectorVariant } from "./Inspector";
import { TopBarProvider, TopBar } from "./TopBar";
import { motion, AnimatePresence } from "framer-motion";
import {
  ExplorerProvider,
  useExplorer,
  Sidebar,
  getSpaceItemKeyFromRoute,
} from "./components/Explorer";
import {
  SelectionProvider,
  useSelection,
} from "./components/Explorer/SelectionContext";
import { KeyboardHandler } from "./components/Explorer/KeyboardHandler";
import { TagAssignmentMode } from "./components/Explorer/TagAssignmentMode";
import { SpacesSidebar } from "./components/SpacesSidebar";
import {
  QuickPreviewFullscreen,
  PREVIEW_LAYER_ID,
} from "./components/QuickPreview";
import { createExplorerRouter } from "./router";
import { useNormalizedQuery, useLibraryMutation } from "./context";
import { usePlatform } from "./platform";
import type { LocationInfo } from "@sd/ts-client";
import { DndContext, DragOverlay, PointerSensor, useSensor, useSensors, pointerWithin, rectIntersection } from "@dnd-kit/core";
import type { CollisionDetection } from "@dnd-kit/core";
import { useState } from "react";
import type { File } from "@sd/ts-client";
import { File as FileComponent } from "./components/Explorer/File";
import { DaemonDisconnectedOverlay } from "./components/DaemonDisconnectedOverlay";

interface AppProps {
  client: SpacedriveClient;
}

export function ExplorerLayout() {
  const location = useLocation();
  const params = useParams();
  const platform = usePlatform();
  const {
    sidebarVisible,
    inspectorVisible,
    setInspectorVisible,
    quickPreviewFileId,
    setQuickPreviewFileId,
    closeQuickPreview,
    currentFiles,
    tagModeActive,
    setTagModeActive,
    viewMode,
    setSpaceItemId,
  } = useExplorer();
  const { selectedFiles, selectFile } = useSelection();

  // Sync route with explorer context for view preferences
  useEffect(() => {
    const spaceItemKey = getSpaceItemKeyFromRoute(
      location.pathname,
      location.search,
    );
    setSpaceItemId(spaceItemKey);
  }, [location.pathname, location.search, setSpaceItemId]);

  // Sync QuickPreview with selection - Explorer is source of truth
  useEffect(() => {
    if (!quickPreviewFileId) return;

    // When selection changes and QuickPreview is open, update preview to match selection
    if (selectedFiles.length === 1 && selectedFiles[0].id !== quickPreviewFileId) {
      setQuickPreviewFileId(selectedFiles[0].id);
    }
  }, [selectedFiles, quickPreviewFileId, setQuickPreviewFileId]);

  // Check if we're on Overview (hide inspector) or in Knowledge view (has its own inspector)
  const isOverview = location.pathname === "/";
  const isKnowledgeView = viewMode === "knowledge";

  // Fetch locations to get current location info
  const locationsQuery = useNormalizedQuery<
    null,
    { locations: LocationInfo[] }
  >({
    wireMethod: "query:locations.list",
    input: null,
    resourceType: "location",
  });

  // Get current location if we're on a location route
  const currentLocation = useMemo(() => {
    if (!params.locationId || !locationsQuery.data?.locations) return null;
    return (
      locationsQuery.data.locations.find(
        (loc) => loc.id === params.locationId,
      ) || null
    );
  }, [params.locationId, locationsQuery.data]);

  useEffect(() => {
    // Listen for inspector window close events
    if (!platform.onWindowEvent) return;

    let unlisten: (() => void) | undefined;

    (async () => {
      try {
        unlisten = await platform.onWindowEvent(
          "inspector-window-closed",
          () => {
            // Show embedded inspector when floating window closes
            setInspectorVisible(true);
          },
        );
      } catch (err) {
        console.error("Failed to setup inspector close listener:", err);
      }
    })();

    return () => {
      unlisten?.();
    };
  }, [platform, setInspectorVisible]);

  const handlePopOutInspector = async () => {
    if (!platform.showWindow) return;

    try {
      await platform.showWindow({
        type: "Inspector",
        item_id: null,
      });
      // Hide the embedded inspector when popped out
      setInspectorVisible(false);
    } catch (err) {
      console.error("Failed to pop out inspector:", err);
    }
  };

  const isPreviewActive = !!quickPreviewFileId;

  return (
    <div className="relative flex h-screen select-none overflow-hidden text-sidebar-ink bg-app rounded-[10px] border border-transparent frame">
      {/* Preview layer - portal target for fullscreen preview, sits between content and sidebar/inspector */}
      <div
        id={PREVIEW_LAYER_ID}
        className="absolute inset-0 z-40 pointer-events-none [&>*]:pointer-events-auto"
      />

      <TopBar
        sidebarWidth={sidebarVisible ? 224 : 0}
        inspectorWidth={
          inspectorVisible && !isOverview && !isKnowledgeView ? 284 : 0
        }
        isPreviewActive={isPreviewActive}
      />

      <AnimatePresence initial={false} mode="popLayout">
        {sidebarVisible && (
          <motion.div
            initial={{ x: -220, width: 0 }}
            animate={{ x: 0, width: 220 }}
            exit={{ x: -220, width: 0 }}
            transition={{ duration: 0.3, ease: [0.25, 1, 0.5, 1] }}
            className="relative z-50 overflow-hidden"
          >
            <SpacesSidebar isPreviewActive={isPreviewActive} />
          </motion.div>
        )}
      </AnimatePresence>

      <div className="relative flex-1 overflow-hidden z-30">
        {/* Router content renders here */}
        <Outlet />

        {/* Tag Assignment Mode - positioned at bottom of main content area */}
        <TagAssignmentMode
          isActive={tagModeActive}
          onExit={() => setTagModeActive(false)}
        />
      </div>

      {/* Keyboard handler (invisible, doesn't cause parent rerenders) */}
      <KeyboardHandler />

      <AnimatePresence initial={false}>
        {/* Hide inspector on Overview screen and Knowledge view (has its own) */}
        {inspectorVisible && !isOverview && !isKnowledgeView && (
          <motion.div
            initial={{ width: 0 }}
            animate={{ width: 280 }}
            exit={{ width: 0 }}
            transition={{ duration: 0.3, ease: [0.25, 1, 0.5, 1] }}
            className="relative z-50 overflow-hidden"
          >
            <div className="w-[280px] min-w-[280px] flex flex-col h-full p-2 bg-transparent">
              <Inspector
                currentLocation={currentLocation}
                onPopOut={handlePopOutInspector}
                isPreviewActive={isPreviewActive}
              />
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Quick Preview - renders via portal into preview layer */}
      {quickPreviewFileId && (() => {
        const currentIndex = currentFiles.findIndex(f => f.id === quickPreviewFileId);
        const hasPrevious = currentIndex > 0;
        const hasNext = currentIndex < currentFiles.length - 1;

        const handleNext = () => {
          if (hasNext && currentFiles[currentIndex + 1]) {
            selectFile(currentFiles[currentIndex + 1], currentFiles, false, false);
          }
        };

        const handlePrevious = () => {
          if (hasPrevious && currentFiles[currentIndex - 1]) {
            selectFile(currentFiles[currentIndex - 1], currentFiles, false, false);
          }
        };

        return (
          <QuickPreviewFullscreen
            fileId={quickPreviewFileId}
            isOpen={!!quickPreviewFileId}
            onClose={closeQuickPreview}
            onNext={handleNext}
            onPrevious={handlePrevious}
            hasPrevious={hasPrevious}
            hasNext={hasNext}
            sidebarWidth={sidebarVisible ? 220 : 0}
            inspectorWidth={
              inspectorVisible && !isOverview && !isKnowledgeView ? 280 : 0
            }
          />
        );
      })()}
    </div>
  );
}

/**
 * DndWrapper - Global drag-and-drop coordinator
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
function DndWrapper({ children }: { children: React.ReactNode }) {
  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8, // Require 8px movement before activating drag
      },
    })
  );
  const addItem = useLibraryMutation("spaces.add_item");
  const [activeItem, setActiveItem] = useState<any>(null);

  // Custom collision detection: prefer -top zones over -bottom zones to avoid double lines
  const customCollision: CollisionDetection = (args) => {
    const collisions = pointerWithin(args);
    if (!collisions || collisions.length === 0) return collisions;

    // If we have multiple collisions, prefer -top over -bottom
    const hasTop = collisions.find(c => String(c.id).endsWith('-top'));
    const hasMiddle = collisions.find(c => String(c.id).endsWith('-middle'));

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

    if (!over || !active.data.current) return;

    const dragData = active.data.current;
    const dropData = over.data.current;

    if (!dragData || dragData.type !== "explorer-file") return;

    // Insert before/after sidebar items (adds item to space/group)
    if (dropData?.action === "insert-before" || dropData?.action === "insert-after") {
      if (!dropData.spaceId) return;

      try {
        await addItem.mutateAsync({
          space_id: dropData.spaceId,
          group_id: dropData.groupId || null,
          item_type: { Path: { sd_path: dragData.sdPath } },
        });
        // TODO: Implement proper ordering relative to itemId
      } catch (err) {
        console.error("Failed to add item:", err);
      }
      return;
    }

    // Move file into location/volume/folder
    if (dropData?.action === "move-into") {
      // TODO: Implement with files.move mutation based on targetType
      // - location: Use targetPath
      // - volume: Look up volume root path
      // - folder: Use targetPath from Path item
      return;
    }

    // Drop on space root area (adds to space)
    if (dropData?.type === "space" && dragData.type === "explorer-file") {
      try {
        await addItem.mutateAsync({
          space_id: dropData.spaceId,
          group_id: null,
          item_type: { Path: { sd_path: dragData.sdPath } },
        });
      } catch (err) {
        console.error("Failed to add item:", err);
      }
    }

    // Drop on group area (adds to group)
    if (dropData?.type === "group" && dragData.type === "explorer-file") {
      try {
        await addItem.mutateAsync({
          space_id: dropData.spaceId,
          group_id: dropData.groupId,
          item_type: { Path: { sd_path: dragData.sdPath } },
        });
      } catch (err) {
        console.error("Failed to add item to group:", err);
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
        {activeItem?.file && activeItem.gridSize ? (
          <div style={{ width: activeItem.gridSize }}>
            <div className="flex flex-col items-center gap-2 p-1 rounded-lg">
              <div className="rounded-lg p-2">
                <FileComponent.Thumb file={activeItem.file} size={Math.max(activeItem.gridSize * 0.6, 60)} />
              </div>
              <div className="text-sm truncate px-2 py-0.5 rounded-md bg-accent text-white max-w-full">
                {activeItem.name}
              </div>
            </div>
          </div>
        ) : null}
      </DragOverlay>
    </DndContext>
  );
}

export function Explorer({ client }: AppProps) {
  const router = createExplorerRouter();

  return (
    <SpacedriveProvider client={client}>
      <DndWrapper>
        <TopBarProvider>
          <SelectionProvider>
            <ExplorerProvider>
              <RouterProvider router={router} />
            </ExplorerProvider>
          </SelectionProvider>
        </TopBarProvider>
      </DndWrapper>
      <DaemonDisconnectedOverlay />
      <Dialogs />
      <ReactQueryDevtools initialIsOpen={false} />
    </SpacedriveProvider>
  );
}
