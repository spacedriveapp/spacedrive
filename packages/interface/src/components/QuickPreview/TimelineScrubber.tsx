import type { File } from "@sd/ts-client";
import { memo } from "react";
import { useServer } from "../../contexts/ServerContext";

interface TimelineScrubberProps {
  file: File;
  hoverPercent: number;
  mouseX: number;
  duration: number;
  sidebarWidth?: number;
  inspectorWidth?: number;
}

/**
 * TimelineScrubber - Shows video frame preview when hovering over timeline
 *
 * Uses thumbstrip sprite sheet to display the frame at the hovered position
 * Similar to YouTube's timeline preview feature
 */
export const TimelineScrubber = memo(function TimelineScrubber({
  file,
  hoverPercent,
  mouseX,
  duration,
  sidebarWidth = 0,
  inspectorWidth = 0,
}: TimelineScrubberProps) {
  const { buildSidecarUrl } = useServer();

  // Find thumbstrip sidecar
  const thumbstripSidecar = file.sidecars?.find((s) => s.kind === "thumbstrip");

  if (!thumbstripSidecar) {
    return null;
  }

  // Parse grid dimensions
  const getGridDimensions = (variant: string) => {
    if (variant.includes("detailed")) return { columns: 10, rows: 10 };
    if (variant.includes("mobile")) return { columns: 3, rows: 3 };
    return { columns: 5, rows: 5 };
  };

  const grid = getGridDimensions(thumbstripSidecar.variant);
  const totalFrames = grid.columns * grid.rows;

  // Build thumbstrip URL
  if (!file.content_identity?.uuid) {
    return null;
  }

  const thumbstripUrl = buildSidecarUrl(
    file.content_identity.uuid,
    thumbstripSidecar.kind,
    thumbstripSidecar.variant,
    thumbstripSidecar.format
  );

  if (!thumbstripUrl) {
    return null;
  }

  // Calculate which frame to show
  const frameIndex = Math.min(
    Math.floor(hoverPercent * totalFrames),
    totalFrames - 1
  );

  const row = Math.floor(frameIndex / grid.columns);
  const col = frameIndex % grid.columns;

  // Calculate sprite position
  const spriteX = grid.columns > 1 ? (col / (grid.columns - 1)) * 100 : 0;
  const spriteY = grid.rows > 1 ? (row / (grid.rows - 1)) * 100 : 0;

  // Preview dimensions (fixed width, 16:9 aspect ratio)
  const previewWidth = 160;
  const previewHeight = 90;

  // Position horizontally following mouse, clamped to controls bounds
  // Adjust for sidebar offset and clamp within the controls area
  const controlsWidth = window.innerWidth - sidebarWidth - inspectorWidth;
  const mouseXRelativeToControls = mouseX - sidebarWidth;
  const leftPosition = Math.max(
    10,
    Math.min(
      mouseXRelativeToControls - previewWidth / 2,
      controlsWidth - previewWidth - 10
    )
  );

  // Format timestamp
  const timestamp = formatTime(hoverPercent * duration);

  return (
    <div
      className="pointer-events-none absolute z-50"
      style={{
        left: leftPosition,
        bottom: 80, // Just above the timeline
        width: previewWidth,
      }}
    >
      {/* Preview frame */}
      <div
        className="overflow-hidden rounded-lg border-2 border-white bg-black shadow-2xl"
        style={{
          width: previewWidth,
          height: previewHeight,
          backgroundImage: `url(${thumbstripUrl})`,
          backgroundSize: `${grid.columns * 100}% ${grid.rows * 100}%`,
          backgroundPosition: `${spriteX}% ${spriteY}%`,
          backgroundRepeat: "no-repeat",
          imageRendering: "crisp-edges",
        }}
      />

      {/* Timestamp below preview */}
      <div className="mt-1 flex justify-center">
        <div className="rounded bg-black/90 px-2 py-0.5 font-mono text-white text-xs">
          {timestamp}
        </div>
      </div>

      {/* Pointer arrow */}
      <div className="absolute top-full left-1/2 -translate-x-1/2">
        <div className="size-0 border-t-4 border-t-white/20 border-r-4 border-r-transparent border-l-4 border-l-transparent" />
      </div>
    </div>
  );
});

function formatTime(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);

  if (hours > 0) {
    return `${hours}:${mins.toString().padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
  }
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}
