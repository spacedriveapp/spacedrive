import { useEffect } from "react";
import { useExplorer } from "../context";
import { useSelection } from "../SelectionContext";
import { useNormalizedQuery } from "../../../context";
import type { DirectorySortBy } from "@sd/ts-client";
import { useTypeaheadSearch } from "./useTypeaheadSearch";

export function useExplorerKeyboard() {
  const { currentPath, sortBy, navigateToPath, viewMode, viewSettings, sidebarVisible, inspectorVisible, openQuickPreview, tagModeActive, setTagModeActive } = useExplorer();
  const { selectedFiles, selectFile, selectAll, clearSelection, focusedIndex, setFocusedIndex, setSelectedFiles } = useSelection();

  // Query files for keyboard operations
  const directoryQuery = useNormalizedQuery({
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

  // Typeahead search (disabled for column view - it handles its own)
  const typeahead = useTypeaheadSearch({
    files,
    onMatch: (file, index) => {
      setFocusedIndex(index);
      setSelectedFiles([file]);
    },
    enabled: viewMode !== "column",
  });

  useEffect(() => {
    const handleKeyDown = async (e: KeyboardEvent) => {
      // Arrow keys: Navigation
      if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.key)) {
        // Skip views that handle their own keyboard navigation
        if (viewMode === "column" || viewMode === "media" || viewMode === "grid") {
          return;
        }

        e.preventDefault();

        if (files.length === 0) return;

        let newIndex = focusedIndex;

        if (viewMode === "list") {
          // List view: only up/down
          if (e.key === "ArrowUp") newIndex = Math.max(0, focusedIndex - 1);
          if (e.key === "ArrowDown") newIndex = Math.min(files.length - 1, focusedIndex + 1);
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

      // Enter: Navigate into directory
      if (e.key === "Enter" && selectedFiles.length === 1) {
        const selected = selectedFiles[0];
        if (selected.kind === "Directory") {
          e.preventDefault();
          navigateToPath(selected.sd_path);
        }
        return;
      }

      // T: Enter tag assignment mode
      if (e.key === "t" && !e.metaKey && !e.ctrlKey && !tagModeActive) {
        e.preventDefault();
        setTagModeActive(true);
        return;
      }

      // Escape: Clear selection
      if (e.code === "Escape" && selectedFiles.length > 0) {
        clearSelection();
      }

      // Typeahead search (handled by hook, disabled for column view)
      typeahead.handleKey(e);
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      typeahead.cleanup();
    };
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
    navigateToPath,
    setFocusedIndex,
    setSelectedFiles,
    openQuickPreview,
  ]);
}
