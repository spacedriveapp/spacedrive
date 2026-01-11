import type { SpeedSample } from "../hooks/useJobs";

interface SpeedGraphProps {
  jobId: string;
  speedHistory: SpeedSample[];
}

/**
 * Placeholder for Windows-style speed graph.
 *
 * TODO: Design with James
 * - Graph library choice (Recharts, Victory, D3, custom Canvas?)
 * - Time range (last N seconds, full transfer, adaptive?)
 * - Y-axis scaling (fixed max, dynamic, logarithmic?)
 * - Show average/peak speed annotations?
 * - Color scheme (gradient, solid, theme-aware?)
 *
 * This component will display a live graph showing transfer speed over time,
 * with real-time updates as new samples arrive via JobProgress events.
 */
export function SpeedGraph({ jobId, speedHistory }: SpeedGraphProps) {
  // Early return if no data
  if (speedHistory.length === 0) {
    return (
      <div className="h-24 bg-app-darkBox rounded-lg flex items-center justify-center">
        <span className="text-xs text-ink-faint">No speed data yet</span>
      </div>
    );
  }

  // Calculate some basic stats for the placeholder
  const rates = speedHistory.map(s => s.bytesPerSecond);
  const maxRate = Math.max(...rates);
  const avgRate = rates.reduce((a, b) => a + b, 0) / rates.length;

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-xs text-ink-faint">Transfer Speed</span>
        <div className="flex gap-3 text-[10px]">
          <span className="text-ink-faint">
            Avg: <span className="text-ink font-medium">{formatSpeed(avgRate)}</span>
          </span>
          <span className="text-ink-faint">
            Peak: <span className="text-ink font-medium">{formatSpeed(maxRate)}</span>
          </span>
        </div>
      </div>

      {/* Placeholder graph area */}
      <div className="h-24 bg-app-darkBox rounded-lg flex items-center justify-center border border-app-line/30">
        <div className="text-center space-y-1">
          <span className="text-xs text-ink-faint block">
            Speed graph (design pending)
          </span>
          <span className="text-[10px] text-ink-faint/50">
            {speedHistory.length} samples collected
          </span>
        </div>
      </div>
    </div>
  );
}

// Format speed (bytes/sec) to human readable
function formatSpeed(bytesPerSecond: number): string {
  const units = ["B/s", "KB/s", "MB/s", "GB/s"];
  let size = bytesPerSecond;
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }

  return `${size.toFixed(unitIndex === 0 ? 0 : 2)} ${units[unitIndex]}`;
}
