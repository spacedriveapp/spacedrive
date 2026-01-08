import { Dialogs } from "@sd/ui";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { RouterProvider } from "react-router-dom";
import { DndProvider } from "./components/DndProvider";
import { DaemonDisconnectedOverlay } from "./components/overlays/DaemonDisconnectedOverlay";
import { DaemonStartupOverlay } from "./components/overlays/DaemonStartupOverlay";
import {
  TabKeyboardHandler,
  TabManagerProvider,
  useTabManager,
} from "./components/TabManager";
import { usePlatform } from "./contexts/PlatformContext";
import { ServerProvider } from "./contexts/ServerContext";
import {
  type SpacedriveClient,
  SpacedriveProvider,
} from "./contexts/SpacedriveContext";
import { useDaemonStatus } from "./hooks/useDaemonStatus";
import { explorerRoutes } from "./router";

interface ShellProps {
  client: SpacedriveClient;
}

function ShellWithTabs() {
  const { router } = useTabManager();

  return (
    <DndProvider>
      <RouterProvider router={router} />
    </DndProvider>
  );
}

/**
 * Tauri-specific wrapper that prevents Shell from rendering until daemon is connected.
 * This avoids the connection storm where hundreds of queries try to execute before daemon is ready.
 */
function ShellWithDaemonCheck() {
  const daemonStatus = useDaemonStatus();
  const { isConnected, isStarting } = daemonStatus;

  return (
    <>
      {isConnected ? (
        // Daemon connected - render full app
        <>
          <TabManagerProvider routes={explorerRoutes}>
            <TabKeyboardHandler />
            <ShellWithTabs />
          </TabManagerProvider>
          <Dialogs />
          <ReactQueryDevtools
            buttonPosition="bottom-right"
            initialIsOpen={false}
          />
        </>
      ) : (
        // Daemon not connected - show appropriate overlay
        <>
          <DaemonStartupOverlay show={isStarting} />
          {!isStarting && (
            <DaemonDisconnectedOverlay daemonStatus={daemonStatus} />
          )}
        </>
      )}
    </>
  );
}

export function Shell({ client }: ShellProps) {
  const platform = usePlatform();
  const isTauri = platform.platform === "tauri";

  return (
    <SpacedriveProvider client={client}>
      <ServerProvider>
        {isTauri ? (
          // Tauri: Wait for daemon connection before rendering content
          <ShellWithDaemonCheck />
        ) : (
          // Web: Render immediately (daemon connection handled differently)
          <>
            <TabManagerProvider routes={explorerRoutes}>
              <TabKeyboardHandler />
              <ShellWithTabs />
            </TabManagerProvider>
            <Dialogs />
            <ReactQueryDevtools
              buttonPosition="bottom-right"
              initialIsOpen={false}
            />
          </>
        )}
      </ServerProvider>
    </SpacedriveProvider>
  );
}
