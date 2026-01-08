import type { SpaceItem as SpaceItemType } from "@sd/ts-client";
import { useLocation } from "react-router-dom";
import { useExplorer } from "../../../routes/explorer/context";

interface UseSpaceItemActiveOptions {
  item: SpaceItemType;
  path: string | null;
  hasCustomOnClick: boolean;
}

/**
 * Determines if a space item is currently "active" (selected/highlighted).
 *
 * Active state is determined by matching the current route/view to the item:
 * - Virtual views (devices) match by view type and ID
 * - Explorer routes match by comparing SD paths
 * - Special routes (/, /recents, etc.) match by exact pathname
 */
export function useSpaceItemActive({
  item,
  path,
  hasCustomOnClick,
}: UseSpaceItemActiveOptions): boolean {
  const location = useLocation();
  const { currentView, currentPath } = useExplorer();

  // Items with custom onClick represent virtual views (like device views).
  // They should ONLY match via virtual view state, never path-based matching.
  if (hasCustomOnClick) {
    if (!currentView) return false;

    const itemIdStr = String(item.id);
    return currentView.view === "device" && currentView.id === itemIdStr;
  }

  // Check virtual view state for items without custom onClick
  if (currentView) {
    const itemIdStr = String(item.id);
    const isViewMatch =
      currentView.view === "device" && currentView.id === itemIdStr;

    if (isViewMatch) return true;

    // When a virtual view is active, regular items should NOT be active
    // even if their path happens to match. Virtual views own the display.
    return false;
  }

  // Check path-based navigation via explorer context
  // Only use currentPath matching when we're actually on the explorer route
  if (
    location.pathname === "/explorer" &&
    currentPath &&
    path &&
    path.startsWith("/explorer?")
  ) {
    const itemPathParam = new URLSearchParams(path.split("?")[1]).get("path");
    if (itemPathParam) {
      try {
        const itemSdPath = JSON.parse(decodeURIComponent(itemPathParam));
        if (JSON.stringify(currentPath) === JSON.stringify(itemSdPath)) {
          return true;
        }
      } catch {
        // Fall through to URL-based comparison
      }
    }
  }

  if (!path) return false;

  // Special routes (/, /recents, /favorites, etc.): exact pathname match
  if (!path.startsWith("/explorer?")) {
    return location.pathname === path;
  }

  // Explorer routes: compare SD paths via URL
  if (location.pathname === "/explorer") {
    const currentSearchParams = new URLSearchParams(location.search);
    const currentPathParam = currentSearchParams.get("path");
    const itemPathParam = new URLSearchParams(path.split("?")[1]).get("path");

    if (currentPathParam && itemPathParam) {
      try {
        const currentSdPath = JSON.parse(decodeURIComponent(currentPathParam));
        const itemSdPath = JSON.parse(decodeURIComponent(itemPathParam));
        return JSON.stringify(currentSdPath) === JSON.stringify(itemSdPath);
      } catch {
        return currentPathParam === itemPathParam;
      }
    }
  }

  return false;
}
