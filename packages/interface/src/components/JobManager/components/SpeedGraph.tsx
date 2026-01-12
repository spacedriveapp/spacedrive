import { useRef, useEffect, useState } from "react";
import type { SpeedSample } from "../hooks/useJobs";

interface SpeedGraphProps {
  jobId: string;
  speedHistory: SpeedSample[];
}

/**
 * Real-time speed graph showing transfer speed over time.
 * Inspired by Windows file copy dialog with smooth curves and gradient fill.
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

  // Calculate stats
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

      {/* Speed graph */}
      <div className="h-24 bg-app-darkBox rounded-lg border border-app-line/30 overflow-hidden">
        <SpeedGraphVisualization jobId={jobId} speedHistory={speedHistory} maxRate={maxRate} />
      </div>
    </div>
  );
}

function SpeedGraphVisualization({
  jobId,
  speedHistory,
  maxRate
}: {
  jobId: string;
  speedHistory: SpeedSample[];
  maxRate: number;
}) {
  const width = 328; // Match container width minus padding
  const height = 96; // 24 * 4 = 96px (h-24)
  const padding = { top: 8, right: 8, bottom: 8, left: 8 };

  const graphWidth = width - padding.left - padding.right;
  const graphHeight = height - padding.top - padding.bottom;

  // Add 10% headroom to max for better visualization
  const yMax = maxRate * 1.1;

  // Generate points for the line
  const points = speedHistory.map((sample, index) => {
    const x = padding.left + (index / Math.max(speedHistory.length - 1, 1)) * graphWidth;
    const y = padding.top + graphHeight - (sample.bytesPerSecond / yMax) * graphHeight;
    return { x, y, rate: sample.bytesPerSecond };
  });

  // Generate SVG path for smooth curve using quadratic bezier
  const linePath = generateSmoothPath(points);

  // Generate area path (for gradient fill)
  const areaPath = points.length > 0 && linePath
    ? linePath +
      ` L ${points[points.length - 1].x},${height - padding.bottom}` +
      ` L ${points[0].x},${height - padding.bottom} Z`
    : "";

  return (
    <svg width={width} height={height} className="w-full h-full">
      <defs>
        {/* Gradient fill for area under curve */}
        <linearGradient id={`speed-gradient-${jobId}`} x1="0%" y1="0%" x2="0%" y2="100%">
          <stop offset="0%" stopColor="rgb(0, 122, 255)" stopOpacity="0.3" />
          <stop offset="100%" stopColor="rgb(0, 122, 255)" stopOpacity="0.05" />
        </linearGradient>
      </defs>

      {/* Grid lines for reference */}
      {[0.25, 0.5, 0.75].map((fraction) => {
        const y = padding.top + graphHeight * fraction;
        return (
          <line
            key={fraction}
            x1={padding.left}
            y1={y}
            x2={width - padding.right}
            y2={y}
            stroke="rgba(255, 255, 255, 0.05)"
            strokeWidth="1"
          />
        );
      })}

      {/* Area fill */}
      {areaPath && (
        <path
          d={areaPath}
          fill={`url(#speed-gradient-${jobId})`}
        />
      )}

      {/* Line graph */}
      {linePath && (
        <path
          d={linePath}
          fill="none"
          stroke="rgb(0, 122, 255)"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      )}

      {/* Data points (show last point for current value indicator) */}
      {points.length > 0 && (
        <circle
          cx={points[points.length - 1].x}
          cy={points[points.length - 1].y}
          r="3"
          fill="rgb(0, 122, 255)"
          stroke="rgba(0, 0, 0, 0.3)"
          strokeWidth="1.5"
        />
      )}
    </svg>
  );
}

// Generate smooth path using cubic bezier curves (Catmull-Rom inspired)
function generateSmoothPath(points: Array<{ x: number; y: number; rate: number }>): string {
  if (points.length === 0) return "";
  if (points.length === 1) return `M ${points[0].x},${points[0].y}`;
  if (points.length === 2) {
    return `M ${points[0].x},${points[0].y} L ${points[1].x},${points[1].y}`;
  }

  let path = `M ${points[0].x},${points[0].y}`;

  // Use cubic bezier curves for smooth interpolation
  for (let i = 0; i < points.length - 1; i++) {
    const current = points[i];
    const next = points[i + 1];
    const prev = points[i - 1] || current;
    const nextNext = points[i + 2] || next;

    // Calculate control points for smooth curve
    const tension = 0.3; // Adjust smoothness (0 = sharp corners, 0.5 = very smooth)

    const cp1x = current.x + (next.x - prev.x) * tension;
    const cp1y = current.y + (next.y - prev.y) * tension;
    const cp2x = next.x - (nextNext.x - current.x) * tension;
    const cp2y = next.y - (nextNext.y - current.y) * tension;

    path += ` C ${cp1x},${cp1y} ${cp2x},${cp2y} ${next.x},${next.y}`;
  }

  return path;
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
