import { ShinyButton } from "@sd/ui";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { useState } from "react";
import { usePlatform } from "../contexts/PlatformContext";
import type { SpacedriveClient } from "../contexts/SpacedriveContext";
import { SpacedriveProvider } from "../contexts/SpacedriveContext";
import { useAllEvents } from "../hooks/useEvent";
import { useLibraries } from "../hooks/useLibraries";

// Type for library info (will be properly typed later)
interface LibraryInfo {
  id: string;
  name: string;
  path?: string;
  stats?: {
    total_files?: number;
    total_size?: number;
    location_count?: number;
  };
}

interface AppProps {
  client: SpacedriveClient;
}

function LibrariesView() {
  const { data: libraries, isLoading, error, refetch } = useLibraries(true);
  const [lastEvent, setLastEvent] = useState<any>(null);
  const [eventCount, setEventCount] = useState(0);
  const [windowError, setWindowError] = useState<string | null>(null);

  const platform = usePlatform();

  // Listen to all core events
  useAllEvents((event) => {
    setLastEvent(event);
    setEventCount((c) => c + 1);
  });

  async function openWindow(windowType: string, params?: any) {
    try {
      setWindowError(null);
      if (!platform.showWindow) {
        throw new Error("Window management not available on this platform");
      }

      const windowDef =
        windowType === "Settings"
          ? { type: "Settings", page: params }
          : { type: windowType, ...params };

      await platform.showWindow(windowDef);
      setLastEvent({ success: "Window opened", type: windowType });
    } catch (err) {
      const errMsg = err instanceof Error ? err.message : String(err);
      setWindowError(errMsg);
      setLastEvent({ error: "Window open failed", message: errMsg });
    }
  }

  const status = isLoading ? "connecting" : error ? "error" : "connected";

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-gray-950 text-white">
      {/* Header */}
      <div className="border-gray-800 border-b bg-gray-900/50 backdrop-blur">
        <div className="px-6 py-4">
          <div className="flex items-center justify-between">
            <div>
              <h1 className="font-bold text-2xl">Spacedrive V2</h1>
              <p className="text-gray-400 text-sm">
                Multi-window Architecture Demo
              </p>
            </div>
            <div className="flex items-center gap-2">
              <div
                className={`h-2 w-2 rounded-full ${
                  status === "connecting"
                    ? "animate-pulse bg-yellow-500"
                    : status === "connected"
                      ? "bg-green-500"
                      : "bg-red-500"
                }`}
              />
              <span className="text-gray-400 text-xs">
                {status === "connected"
                  ? "Connected"
                  : status === "error"
                    ? "Error"
                    : "Loading"}
              </span>
            </div>
          </div>
        </div>
      </div>

      {/* Main Content */}
      <div className="flex-1 overflow-y-auto">
        <div className="mx-auto max-w-6xl space-y-6 p-6">
          {/* Libraries Grid */}
          {status === "connected" && libraries && (
            <div>
              <h2 className="mb-4 font-semibold text-lg">Libraries</h2>
              <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
                {libraries.length === 0 ? (
                  <div className="col-span-full rounded-lg border border-gray-800 bg-gray-900 p-8 text-center">
                    <p className="text-gray-400">No libraries found</p>
                  </div>
                ) : (
                  libraries.map((lib: LibraryInfo) => (
                    <div
                      className="rounded-lg border border-gray-800 bg-gray-900 p-4 transition-colors hover:border-gray-700"
                      key={lib.id}
                    >
                      <h3 className="mb-3 font-semibold">{lib.name}</h3>
                      <div className="space-y-2 text-sm">
                        <div className="flex justify-between text-gray-400">
                          <span>Files</span>
                          <span className="text-white">
                            {lib.stats?.total_files?.toLocaleString() || 0}
                          </span>
                        </div>
                        <div className="flex justify-between text-gray-400">
                          <span>Size</span>
                          <span className="text-white">
                            {formatBytes(lib.stats?.total_size || 0)}
                          </span>
                        </div>
                        {lib.path && (
                          <div className="border-gray-800 border-t pt-2">
                            <p
                              className="truncate text-gray-500 text-xs"
                              title={lib.path}
                            >
                              {lib.path}
                            </p>
                          </div>
                        )}
                      </div>
                    </div>
                  ))
                )}
              </div>
            </div>
          )}

          {/* Events Section */}
          <div>
            <div className="mb-4 flex items-center justify-between">
              <h2 className="font-semibold text-lg">Live Events</h2>
              <span className="rounded-full bg-gray-800 px-2 py-1 text-xs">
                {eventCount} received
              </span>
            </div>
            <div className="rounded-lg border border-gray-800 bg-gray-900 p-4">
              {lastEvent ? (
                <pre className="max-h-48 overflow-auto font-mono text-gray-400 text-xs">
                  {JSON.stringify(lastEvent, null, 2)}
                </pre>
              ) : (
                <p className="py-4 text-center text-gray-500 text-sm">
                  Waiting for events...
                </p>
              )}
            </div>
          </div>

          {/* Window Controls */}
          <div>
            <h2 className="mb-4 font-semibold text-lg">Window Management</h2>
            <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
              <ShinyButton
                disabled={isLoading}
                onClick={() => refetch()}
                variant="accent"
              >
                Refresh
              </ShinyButton>
              <ShinyButton onClick={() => openWindow("Settings", "general")}>
                Settings
              </ShinyButton>
              <ShinyButton
                onClick={() =>
                  openWindow("Inspector", {
                    item_id: "test-123",
                  })
                }
              >
                Inspector
              </ShinyButton>
              <ShinyButton onClick={() => openWindow("FloatingControls")}>
                Floating Controls
              </ShinyButton>
            </div>
            {windowError && (
              <div className="mt-3 rounded-lg border border-red-500 bg-red-900/20 p-3">
                <p className="text-red-400 text-sm">{windowError}</p>
              </div>
            )}
          </div>
        </div>
      </div>

      {error && (
        <div className="absolute inset-0 flex items-center justify-center bg-black/50 backdrop-blur">
          <div className="max-w-md rounded-lg border border-red-500 bg-gray-900 p-6">
            <h2 className="mb-2 font-bold text-red-400 text-xl">Error</h2>
            <p className="text-gray-300 text-sm">{error.message}</p>
          </div>
        </div>
      )}
    </div>
  );
}

export function DemoWindow({ client }: AppProps) {
  return (
    <SpacedriveProvider client={client}>
      <LibrariesView />
      <ReactQueryDevtools buttonPosition="bottom-right" initialIsOpen={false} />
    </SpacedriveProvider>
  );
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / k ** i).toFixed(1)} ${sizes[i]}`;
}
