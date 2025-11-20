import { useEffect } from "react";
import { useExplorer } from "../context";
import { useSelection } from "../SelectionContext";
import { useNormalizedCache } from "../../../context";
import type { DirectorySortBy } from "@sd/ts-client/generated/types";

export function useExplorerKeyboard() {
  const { currentPath, sortBy, setCurrentPath, viewMode, viewSettings, sidebarVisible, inspectorVisible, openQuickPreview } = useExplorer();
  const { selectedFiles, selectFile, selectAll, clearSelection, focusedIndex, setFocusedIndex, setSelectedFiles } = useSelection();

  // Query files for keyboard operations
  const directoryQuery = useNormalizedCache({
    wireMethod: "query:files.directory_listing",
    input: currentPath
      ? {
          path: currentPath,
          limit: null,
          include_hidden: false,
          sort_by: sortBy as DirectorySortBy,
        }
      : null!,
    resourceType: "file",
    enabled: !!currentPath,
    pathScope: currentPath ?? undefined,
  });

  const files = directoryQuery.data?.files || [];

  useEffect(() => {
    const handleKeyDown = async (e: KeyboardEvent) => {
      // Arrow keys: Navigation
      if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.key)) {
        e.preventDefault();

        if (files.length === 0) return;

        let newIndex = focusedIndex;

        if (viewMode === "list") {
          // List view: only up/down
          if (e.key === "ArrowUp") newIndex = Math.max(0, focusedIndex - 1);
          if (e.key === "ArrowDown") newIndex = Math.min(files.length - 1, focusedIndex + 1);
        } else if (viewMode === "column") {
          // Column view: up/down in current column, left/right between columns (TODO)
          if (e.key === "ArrowUp") newIndex = Math.max(0, focusedIndex - 1);
          if (e.key === "ArrowDown") newIndex = Math.min(files.length - 1, focusedIndex + 1);
          // Left/right for column navigation - TODO: implement column switching
        } else if (viewMode === "grid" || viewMode === "media") {
          // Grid/Media view: 2D navigation
          const containerWidth =
            window.innerWidth -
            (sidebarVisible ? 224 : 0) -
            (inspectorVisible ? 284 : 0) -
            48;
          const itemWidth = viewSettings.gridSize + viewSettings.gapSize;
          const columns = Math.floor(containerWidth / itemWidth);

          if (e.key === "ArrowUp") newIndex = Math.max(0, focusedIndex - columns);
          if (e.key === "ArrowDown") newIndex = Math.min(files.length - 1, focusedIndex + columns);
          if (e.key === "ArrowLeft") newIndex = Math.max(0, focusedIndex - 1);
          if (e.key === "ArrowRight") newIndex = Math.min(files.length - 1, focusedIndex + 1);
        }

        if (newIndex !== focusedIndex) {
          setFocusedIndex(newIndex);
          setSelectedFiles([files[newIndex]]);
        }
        return;
      }

      // Cmd/Ctrl+A: Select all
      if ((e.metaKey || e.ctrlKey) && e.key === "a") {
        e.preventDefault();
        selectAll(files);
        return;
      }

      // Spacebar: Open Quick Preview (in-app modal)
      if (e.code === "Space" && selectedFiles.length === 1) {
        e.preventDefault();
        openQuickPreview(selectedFiles[0].id);
        return;
      }

      // Enter: Navigate into directory (for column view)
      if (e.key === "Enter" && selectedFiles.length === 1) {
        const selected = selectedFiles[0];
        if (selected.kind === "Directory") {
          e.preventDefault();
          setCurrentPath(selected.sd_path);
        }
        return;
      }

      // Escape: Clear selection
      if (e.code === "Escape" && selectedFiles.length > 0) {
        clearSelection();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [
    selectedFiles,
    files,
    focusedIndex,
    viewMode,
    viewSettings,
    sidebarVisible,
    inspectorVisible,
    selectAll,
    clearSelection,
    setCurrentPath,
    setFocusedIndex,
    setSelectedFiles,
    openQuickPreview,
  ]);
}
