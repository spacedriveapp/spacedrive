import clsx from "clsx";
import { formatBytes } from "../utils";

interface VolumeSizeBarProps {
  totalBytes: number;
  availableBytes: number;
  className?: string;
}

/**
 * Visual size bar for volumes showing used/available space
 */
export function VolumeSizeBar({
  totalBytes,
  availableBytes,
  className,
}: VolumeSizeBarProps) {
  const usedBytes = totalBytes - availableBytes;
  const usedPercentage = (usedBytes / totalBytes) * 100;

  return (
    <div className={clsx("w-full px-2 py-1", className)}>
      {/* Size bar */}
      <div className="relative mb-1.5 h-1.5 overflow-hidden rounded-full bg-app-box">
        <div
          className="h-full rounded-full bg-accent transition-all"
          style={{ width: `${usedPercentage}%` }}
        />
      </div>

      {/* Size text */}
      <div className="flex items-center justify-between px-0.5 text-[10px] text-ink-dull">
        <span>{formatBytes(availableBytes)} free</span>
        <span>{formatBytes(totalBytes)}</span>
      </div>
    </div>
  );
}
