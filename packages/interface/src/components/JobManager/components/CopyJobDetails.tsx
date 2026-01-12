import type { JobListItem } from "../types";
import type { SpeedSample } from "../hooks/useJobs";
import { SpeedGraph } from "./SpeedGraph";
import { formatDuration } from "../types";

/**
 * Map strategy name to display label (enables i18n in future)
 */
function getStrategyLabel(strategyName: string | undefined, isMove: boolean): string {
  if (!strategyName) return "Unknown method";

  switch (strategyName) {
    case "RemoteTransfer":
      return isMove ? "Network move" : "Network copy";
    case "LocalMove":
      return "Atomic move";
    case "FastCopy":
      return "Fast copy";
    case "LocalStream":
      return isMove ? "Streaming move" : "Streaming copy";
    default:
      return strategyName;
  }
}

interface CopyJobDetailsProps {
  job: JobListItem;
  speedHistory: SpeedSample[];
}

export function CopyJobDetails({ job, speedHistory }: CopyJobDetailsProps) {
  const generic = job.generic_progress;

  if (!generic) {
    return (
      <div className="p-4 text-xs text-ink-faint">
        No progress data available
      </div>
    );
  }

  // Extract copy metadata from generic progress
  const metadata = generic.metadata as any;
  const isMove = job.action_context?.action_type === "files.move";
  const strategyName = metadata?.strategy?.strategy_name;
  const strategyLabel = getStrategyLabel(strategyName, isMove);
  const isCrossDevice = metadata?.strategy?.is_cross_device || false;
  const isCrossVolume = metadata?.strategy?.is_cross_volume || false;

  // Format current file path from SdPath
  const currentPath = generic.current_path
    ? formatSdPath(generic.current_path)
    : null;

  // Calculate formatted values
  const filesProgress = `${generic.completion?.completed || 0} / ${generic.completion?.total || 0}`;
  const bytesCompleted = generic.completion?.bytes_completed || 0;
  const totalBytes = generic.completion?.total_bytes || 0;
  const bytesProgress = totalBytes > 0
    ? `${formatBytes(bytesCompleted)} / ${formatBytes(totalBytes)}`
    : `${formatBytes(bytesCompleted)} / calculating...`;
  const speed = generic.performance?.rate ? formatSpeed(generic.performance.rate) : "—";
  const eta = generic.performance?.estimated_remaining
    ? formatDurationSeconds(generic.performance.estimated_remaining.secs)
    : totalBytes === 0 ? "calculating..." : "—";

  return (
    <div className="p-4 space-y-3">
      {/* Transfer method info */}
      <div className="flex items-center gap-2">
        <span className="text-xs text-ink-faint">Method:</span>
        <span className="text-xs font-medium text-ink">{strategyLabel}</span>
      </div>

      {/* Current file */}
      {currentPath && (
        <div>
          <span className="text-xs text-ink-faint">Current file:</span>
          <div className="mt-1 text-xs text-ink truncate font-mono bg-app-darkBox rounded px-2 py-1">
            {currentPath}
          </div>
        </div>
      )}

      {/* Progress stats grid */}
      <div className="grid grid-cols-2 gap-3">
        <Stat label="Files" value={filesProgress} />
        <Stat label="Size" value={bytesProgress} />
        <Stat label="Speed" value={speed} />
        <Stat label="ETA" value={eta} />
      </div>

      {/* Speed graph */}
      <SpeedGraph jobId={job.id} speedHistory={speedHistory} />
    </div>
  );
}

// Stat component for grid
function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-[10px] text-ink-faint uppercase tracking-wide">{label}</span>
      <span className="text-xs font-medium text-ink">{value}</span>
    </div>
  );
}

// Format SdPath to human-readable string
function formatSdPath(path: any): string {
  if (typeof path === "string") {
    return path;
  }

  // Handle Physical path: { Physical: { device_slug: "...", path: "..." } }
  if (path?.Physical?.path) {
    const pathStr = path.Physical.path;
    // Show home directory as ~
    return pathStr.replace(/^\/Users\/[^/]+/, "~");
  }

  // Handle Local path: { Local: { path: "..." } }
  if (path?.Local?.path) {
    return path.Local.path.replace(/^\/Users\/[^/]+/, "~");
  }

  // Fallback to JSON string
  return JSON.stringify(path);
}

// Format bytes to human readable
function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = bytes;
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }

  return `${size.toFixed(unitIndex === 0 ? 0 : 2)} ${units[unitIndex]}`;
}

// Format speed (bytes/sec) to human readable
function formatSpeed(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`;
}

// Format duration from seconds to human readable
function formatDurationSeconds(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const hrs = Math.floor(mins / 60);

  if (hrs > 0) {
    return `${hrs}h ${mins % 60}m`;
  }
  if (mins > 0) {
    return `${mins}m ${Math.floor(seconds % 60)}s`;
  }
  return `${Math.floor(seconds)}s`;
}
