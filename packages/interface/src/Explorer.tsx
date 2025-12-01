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
import { ExplorerProvider, useExplorer, Sidebar, getSpaceItemKeyFromRoute } from "./components/explorer";
import {
  SelectionProvider,
  useSelection,
} from "./components/Explorer/SelectionContext";
import { KeyboardHandler } from "./components/Explorer/KeyboardHandler";
import { TagAssignmentMode } from "./components/Explorer/TagAssignmentMode";
import { SpacesSidebar } from "./components/SpacesSidebar";
import { QuickPreviewFullscreen, PREVIEW_LAYER_ID } from "./components/QuickPreview";
import { createExplorerRouter } from "./router";
import { useNormalizedCache } from "./context";
import { usePlatform } from "./platform";
import type { LocationInfo } from "@sd/ts-client";

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
    closeQuickPreview,
    goToNextPreview,
    goToPreviousPreview,
    tagModeActive,
    setTagModeActive,
    viewMode,
    setSpaceItemId,
  } = useExplorer();

  // Sync route with explorer context for view preferences
  useEffect(() => {
    const spaceItemKey = getSpaceItemKeyFromRoute(location.pathname, location.search);
    setSpaceItemId(spaceItemKey);
  }, [location.pathname, location.search, setSpaceItemId]);

  // Check if we're on Overview (hide inspector) or in Knowledge view (has its own inspector)
  const isOverview = location.pathname === "/";
  const isKnowledgeView = viewMode === "knowledge";

  // Fetch locations to get current location info
  const locationsQuery = useNormalizedCache<
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
        inspectorWidth={inspectorVisible && !isOverview && !isKnowledgeView ? 284 : 0}
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
      {quickPreviewFileId && (
        <QuickPreviewFullscreen
          fileId={quickPreviewFileId}
          isOpen={!!quickPreviewFileId}
          onClose={closeQuickPreview}
          onNext={() => goToNextPreview([])}
          onPrevious={() => goToPreviousPreview([])}
          hasPrevious={false}
          hasNext={false}
          sidebarWidth={sidebarVisible ? 220 : 0}
          inspectorWidth={inspectorVisible && !isOverview && !isKnowledgeView ? 280 : 0}
        />
      )}
    </div>
  );
}

export function Explorer({ client }: AppProps) {
  const router = createExplorerRouter();

  return (
    <SpacedriveProvider client={client}>
      <TopBarProvider>
        <SelectionProvider>
          <ExplorerProvider>
            <RouterProvider router={router} />
          </ExplorerProvider>
        </SelectionProvider>
      </TopBarProvider>
      <Dialogs />
      <ReactQueryDevtools initialIsOpen={false} />
    </SpacedriveProvider>
  );
}
