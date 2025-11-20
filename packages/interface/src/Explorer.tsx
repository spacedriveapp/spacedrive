import { SpacedriveProvider, type SpacedriveClient } from "./context";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { RouterProvider, Outlet, useLocation, useParams } from "react-router-dom";
import { useEffect, useMemo } from "react";
import { Dialogs } from "@sd/ui";
import { Inspector, type InspectorVariant } from "./Inspector";
import { TopBarProvider, TopBar } from "./TopBar";
import { motion, AnimatePresence } from "framer-motion";
import { ExplorerProvider, useExplorer, Sidebar } from "./components/explorer";
import { SelectionProvider, useSelection } from "./components/Explorer/SelectionContext";
import { SpacesSidebar } from "./components/SpacesSidebar";
import { QuickPreviewModal } from "./components/QuickPreview";
import { createExplorerRouter } from "./router";
import { useNormalizedCache } from "./context";
import { usePlatform } from "./platform";
import type { LocationInfo } from "@sd/ts-client/generated/types";

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
  } = useExplorer();

  // Check if we're on Overview (hide inspector)
  const isOverview = location.pathname === "/";

  // Fetch locations to get current location info
  const locationsQuery = useNormalizedCache<null, { locations: LocationInfo[] }>({
    wireMethod: "query:locations.list",
    input: null,
    resourceType: "location",
  });

  // Get current location if we're on a location route
  const currentLocation = useMemo(() => {
    if (!params.locationId || !locationsQuery.data?.locations) return null;
    return locationsQuery.data.locations.find(loc => loc.id === params.locationId) || null;
  }, [params.locationId, locationsQuery.data]);

  useEffect(() => {
    // Listen for inspector window close events
    if (!platform.onWindowEvent) return;

    let unlisten: (() => void) | undefined;

    (async () => {
      try {
        unlisten = await platform.onWindowEvent("inspector-window-closed", () => {
          // Show embedded inspector when floating window closes
          setInspectorVisible(true);
        });
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

  return (
    <div className="relative flex h-screen select-none overflow-hidden text-sidebar-ink bg-app rounded-[10px] border border-transparent frame">
      <TopBar
        sidebarWidth={sidebarVisible ? 224 : 0}
        inspectorWidth={inspectorVisible && !isOverview ? 284 : 0}
      />

      <AnimatePresence initial={false} mode="popLayout">
        {sidebarVisible && (
          <motion.div
            initial={{ x: -220, width: 0 }}
            animate={{ x: 0, width: 220 }}
            exit={{ x: -220, width: 0 }}
            transition={{ duration: 0.3, ease: [0.25, 1, 0.5, 1] }}
            className="overflow-hidden"
          >
            <SpacesSidebar />
            {/*<Sidebar />*/}
          </motion.div>
        )}
      </AnimatePresence>

      <div className="flex-1 overflow-hidden">
        {/* Router content renders here */}
        <Outlet />
      </div>

      <AnimatePresence initial={false}>
        {/* Hide inspector on Overview screen */}
        {inspectorVisible && !isOverview && (
          <motion.div
            initial={{ width: 0 }}
            animate={{ width: 280 }}
            exit={{ width: 0 }}
            transition={{ duration: 0.3, ease: [0.25, 1, 0.5, 1] }}
            className="overflow-hidden"
          >
            <div className="w-[280px] min-w-[280px] flex flex-col h-full p-2 bg-app">
              <Inspector
                currentLocation={currentLocation}
                onPopOut={handlePopOutInspector}
              />
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Quick Preview Modal - TODO: Fix files reference */}
      {quickPreviewFileId && (
        <QuickPreviewModal
          fileId={quickPreviewFileId}
          isOpen={!!quickPreviewFileId}
          onClose={closeQuickPreview}
          onNext={() => goToNextPreview([])}
          onPrevious={() => goToPreviousPreview([])}
          hasPrevious={false}
          hasNext={false}
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
