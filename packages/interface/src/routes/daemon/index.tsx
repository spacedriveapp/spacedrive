import { Check, CircleNotch, Power, Warning } from "@phosphor-icons/react";
import { useEffect, useState } from "react";
import { usePlatform } from "../../contexts/PlatformContext";

interface DaemonStatus {
  is_running: boolean;
  socket_path: string;
  server_url: string | null;
  started_by_us: boolean;
}

export function DaemonManager() {
  const platform = usePlatform();
  const [status, setStatus] = useState<DaemonStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [autoStart, setAutoStart] = useState(true);
  const [runInProcess, setRunInProcess] = useState(false);
  const [isStarting, setIsStarting] = useState(false);
  const [isStopping, setIsStopping] = useState(false);

  useEffect(() => {
    checkDaemonStatus();
  }, []);

  async function checkDaemonStatus() {
    if (!platform.getDaemonStatus) return;

    setIsLoading(true);
    setError(null);
    try {
      const daemonStatus = await platform.getDaemonStatus();
      setStatus(daemonStatus);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setStatus(null);
    } finally {
      setIsLoading(false);
    }
  }

  async function handleStartDaemon() {
    if (!platform.startDaemonProcess) return;

    setIsStarting(true);
    setError(null);
    try {
      await platform.startDaemonProcess();
      await checkDaemonStatus();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsStarting(false);
    }
  }

  async function handleStopDaemon() {
    if (!platform.stopDaemonProcess) return;

    setIsStopping(true);
    setError(null);
    try {
      await platform.stopDaemonProcess();
      await checkDaemonStatus();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsStopping(false);
    }
  }

  async function handleOpenSettings() {
    if (!platform.openMacOSSettings) return;

    try {
      await platform.openMacOSSettings();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  function getStatusColor() {
    if (isLoading) return "text-ink-faint";
    return status?.is_running ? "text-green-500" : "text-red-500";
  }

  function getStatusIcon() {
    if (isLoading) return CircleNotch;
    return status?.is_running ? Check : Warning;
  }

  const StatusIcon = getStatusIcon();
  const isRunning = status?.is_running;

  return (
    <div className="flex h-full flex-col gap-6 p-6 text-ink">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="font-semibold text-2xl text-ink">Daemon Manager</h1>
          <p className="mt-1 text-ink-dull text-sm">
            Control the Spacedrive daemon process
          </p>
        </div>
      </div>

      {/* Status Card */}
      <div className="rounded-lg border border-app-line bg-app-box p-4">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="font-medium text-ink text-lg">Status</h2>
          <div className="flex items-center gap-2">
            <StatusIcon
              className={`size-5 ${getStatusColor()} ${
                isLoading ? "animate-spin" : ""
              }`}
              weight="fill"
            />
            <span className={`font-medium text-sm ${getStatusColor()}`}>
              {isLoading ? "Checking..." : isRunning ? "Running" : "Stopped"}
            </span>
          </div>
        </div>

        {error && (
          <div className="mb-4 rounded-md border border-red-500/20 bg-red-500/10 p-3">
            <p className="text-red-400 text-sm">{error}</p>
          </div>
        )}

        <div className="space-y-2 text-sm">
          <div className="flex justify-between">
            <span className="text-ink-dull">Socket Path:</span>
            <span className="font-mono text-ink text-xs">
              {status?.socket_path || "N/A"}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-ink-dull">Server URL:</span>
            <span className="font-mono text-ink text-xs">
              {status?.server_url || "N/A"}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-ink-dull">Started by App:</span>
            <span className="text-ink">
              {status?.started_by_us ? "Yes" : "No"}
            </span>
          </div>
        </div>

        <button
          className="mt-4 w-full rounded-md bg-accent px-4 py-2 font-medium text-sm text-white transition-colors hover:bg-accent-deep"
          onClick={checkDaemonStatus}
        >
          Refresh Status
        </button>
      </div>

      {/* Settings Card */}
      <div className="rounded-lg border border-app-line bg-app-box p-4">
        <h2 className="mb-4 font-medium text-ink text-lg">Settings</h2>

        <div className="space-y-4">
          {/* Auto-start Toggle */}
          <div className="flex items-center justify-between">
            <div>
              <h3 className="font-medium text-ink text-sm">
                Auto-start Daemon
              </h3>
              <p className="mt-1 text-ink-dull text-xs">
                Start daemon automatically when app launches
              </p>
            </div>
            <button
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                autoStart ? "bg-accent" : "bg-app-line"
              }`}
              onClick={() => setAutoStart(!autoStart)}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  autoStart ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>

          {/* Run in Process Toggle */}
          <div className="flex items-center justify-between">
            <div>
              <h3 className="font-medium text-ink text-sm">Run in Process</h3>
              <p className="mt-1 text-ink-dull text-xs">
                Run daemon in the app process (fallback if background permission
                denied)
              </p>
            </div>
            <button
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                runInProcess ? "bg-accent" : "bg-app-line"
              }`}
              onClick={() => setRunInProcess(!runInProcess)}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  runInProcess ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>
        </div>
      </div>

      {/* macOS Background Permission Notice */}
      <div className="rounded-lg border border-sidebar-line bg-sidebar-box p-4">
        <div className="flex items-start gap-3">
          <Warning
            className="mt-0.5 size-5 flex-shrink-0 text-yellow-500"
            weight="fill"
          />
          <div>
            <h3 className="font-medium text-ink text-sm">
              macOS Background Items
            </h3>
            <p className="mt-1 text-ink-dull text-xs">
              On macOS, running background processes requires permission. If the
              daemon fails to start automatically, check System Settings →
              General → Login Items & Extensions and allow Spacedrive to run in
              the background.
            </p>
            <button
              className="mt-2 font-medium text-accent text-xs hover:text-accent-deep"
              onClick={handleOpenSettings}
            >
              Open System Settings →
            </button>
          </div>
        </div>
      </div>

      {/* Actions */}
      <div className="flex gap-3">
        <button
          className="flex flex-1 items-center justify-center gap-2 rounded-md bg-green-500 px-4 py-2 font-medium text-sm text-white transition-colors hover:bg-green-600 disabled:bg-app-line disabled:text-ink-faint"
          disabled={isRunning || isStarting || isLoading}
          onClick={handleStartDaemon}
        >
          {isStarting ? (
            <CircleNotch className="size-4 animate-spin" weight="bold" />
          ) : (
            <Power className="size-4" weight="bold" />
          )}
          {isStarting ? "Starting..." : "Start Daemon"}
        </button>
        <button
          className="flex flex-1 items-center justify-center gap-2 rounded-md bg-red-500 px-4 py-2 font-medium text-sm text-white transition-colors hover:bg-red-600 disabled:bg-app-line disabled:text-ink-faint"
          disabled={
            !isRunning || isStopping || isLoading || !status?.started_by_us
          }
          onClick={handleStopDaemon}
        >
          {isStopping ? (
            <CircleNotch className="size-4 animate-spin" weight="bold" />
          ) : (
            <Power className="size-4" weight="bold" />
          )}
          {isStopping ? "Stopping..." : "Stop Daemon"}
        </button>
      </div>
    </div>
  );
}
