import { useEffect, useRef } from "react";
import { useExplorer } from "../context";
import { useSelection } from "../SelectionContext";
import { useNormalizedQuery } from "../../../context";
import type { DirectorySortBy } from "@sd/ts-client";

export function useExplorerKeyboard() {
  const { currentPath, sortBy, setCurrentPath, viewMode, viewSettings, sidebarVisible, inspectorVisible, openQuickPreview, tagModeActive, setTagModeActive } = useExplorer();
  const { selectedFiles, selectFile, selectAll, clearSelection, focusedIndex, setFocusedIndex, setSelectedFiles } = useSelection();

  // Typeahead search state
  const searchStringRef = useRef("");
  const searchTimeoutRef = useRef<NodeJS.Timeout | null>(null);

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

  useEffect(() => {
    const handleKeyDown = async (e: KeyboardEvent) => {
      // Arrow keys: Navigation
      if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.key)) {
        // Skip column view - each column handles its own keyboard navigation
        if (viewMode === "column") {
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
          setCurrentPath(selected.sd_path);
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

      // Typeahead search: Only trigger if:
      // - Single character key
      // - No modifiers (except Shift for capitals)
      // - Not already handled above
      // - Target is not an input element
      const target = e.target as HTMLElement;
      const isInputElement = target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;

      if (
        !isInputElement &&
        e.key.length === 1 &&
        !e.metaKey &&
        !e.ctrlKey &&
        !e.altKey &&
        files.length > 0
      ) {
        // Clear previous timeout
        if (searchTimeoutRef.current) {
          clearTimeout(searchTimeoutRef.current);
        }

        // Update search string
        searchStringRef.current += e.key.toLowerCase();

        // Find first file that matches the search string
        const matchIndex = files.findIndex((file) => {
          const fileName = file.name.toLowerCase();
          return fileName.startsWith(searchStringRef.current);
        });

        // If match found, select it
        if (matchIndex !== -1) {
          setFocusedIndex(matchIndex);
          setSelectedFiles([files[matchIndex]]);

          // Scroll to the matched file
          const element = document.querySelector(`[data-file-id="${files[matchIndex].id}"]`);
          if (element) {
            element.scrollIntoView({ block: "nearest", behavior: "smooth" });
          }
        }

        // Reset search string after 500ms of inactivity
        searchTimeoutRef.current = setTimeout(() => {
          searchStringRef.current = "";
        }, 500);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      if (searchTimeoutRef.current) {
        clearTimeout(searchTimeoutRef.current);
      }
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
    setCurrentPath,
    setFocusedIndex,
    setSelectedFiles,
    openQuickPreview,
  ]);
}
