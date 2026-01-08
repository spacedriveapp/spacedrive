import { memo, useMemo } from "react";
import { TopBarSection } from "./Section";
import { useOverflowCalculation } from "./useOverflowCalculation";

interface TopBarProps {
  sidebarWidth?: number;
  inspectorWidth?: number;
  isPreviewActive?: boolean;
}

// Traffic lights on macOS are ~80px from left edge when sidebar is collapsed
const MACOS_TRAFFIC_LIGHT_WIDTH = 90;

// Detect macOS once
const isMacOS =
  typeof navigator !== "undefined" &&
  (navigator.platform.toLowerCase().includes("mac") ||
    navigator.userAgent.includes("Mac"));

export const TopBar = memo(function TopBar({
  sidebarWidth = 0,
  inspectorWidth = 0,
  isPreviewActive = false,
}: TopBarProps) {
  const containerRef = useOverflowCalculation();

  const isSidebarCollapsed = sidebarWidth === 0;

  // Add padding for macOS traffic lights when sidebar is collapsed
  const leftPadding = useMemo(
    () => (isMacOS && isSidebarCollapsed ? MACOS_TRAFFIC_LIGHT_WIDTH : 0),
    [isSidebarCollapsed]
  );

  return (
    <div
      className="absolute top-0 z-[60] h-12"
      data-tauri-drag-region
      style={{
        left: sidebarWidth,
        right: inspectorWidth,
      }}
    >
      <div
        className="relative flex h-full items-center gap-3 overflow-hidden px-3"
        data-tauri-drag-region
        ref={containerRef}
        style={{
          paddingLeft: leftPadding ? `${leftPadding}px` : undefined,
        }}
      >
        <TopBarSection position="left" />
        <TopBarSection position="center" />
        <TopBarSection position="right" />
      </div>
    </div>
  );
});
