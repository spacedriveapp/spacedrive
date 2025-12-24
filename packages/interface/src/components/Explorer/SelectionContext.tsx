import { createContext, useContext, useState, useCallback, useMemo, useEffect, type ReactNode } from "react";
import { usePlatform } from "../../platform";
import type { File } from "@sd/ts-client";
import { useClipboard } from "../../hooks/useClipboard";

interface SelectionContextValue {
  selectedFiles: File[];
  selectedFileIds: Set<string>;
  isSelected: (fileId: string) => boolean;
  setSelectedFiles: (files: File[]) => void;
  selectFile: (file: File, files: File[], multi?: boolean, range?: boolean) => void;
  clearSelection: () => void;
  selectAll: (files: File[]) => void;
  focusedIndex: number;
  setFocusedIndex: (index: number) => void;
  moveFocus: (direction: "up" | "down" | "left" | "right", files: File[]) => void;
}

const SelectionContext = createContext<SelectionContextValue | null>(null);

interface SelectionProviderProps {
  children: ReactNode;
  isActiveTab?: boolean;
}

export function SelectionProvider({ children, isActiveTab = true }: SelectionProviderProps) {
  const platform = usePlatform();
  const clipboard = useClipboard();
  const [selectedFiles, setSelectedFiles] = useState<File[]>([]);
  const [focusedIndex, setFocusedIndex] = useState(-1);
  const [lastSelectedIndex, setLastSelectedIndex] = useState(-1);

  // Sync selected file IDs to platform (for cross-window state sharing)
  // Only sync for the active tab to avoid conflicts
  useEffect(() => {
    if (!isActiveTab) return;

    const fileIds = selectedFiles.map((f) => f.id);

    if (platform.setSelectedFileIds) {
      platform.setSelectedFileIds(fileIds).catch((err) => {
        console.error("Failed to sync selected files to platform:", err);
      });
    }
  }, [selectedFiles, platform, isActiveTab]);

  // Update native menu items based on selection and clipboard state
  // Only update for active tab
  useEffect(() => {
    if (!isActiveTab) return;

    const hasSelection = selectedFiles.length > 0;
    const isSingleSelection = selectedFiles.length === 1;
    const hasClipboard = clipboard.hasClipboard();

    platform.updateMenuItems?.([
      { id: "copy", enabled: hasSelection },
      { id: "cut", enabled: hasSelection },
      { id: "duplicate", enabled: hasSelection },
      { id: "rename", enabled: isSingleSelection },
      { id: "delete", enabled: hasSelection },
      { id: "paste", enabled: hasClipboard },
    ]);
  }, [selectedFiles, clipboard, platform, isActiveTab]);

  const clearSelection = useCallback(() => {
    setSelectedFiles([]);
    setFocusedIndex(-1);
    setLastSelectedIndex(-1);
  }, []);

  const selectAll = useCallback((files: File[]) => {
    setSelectedFiles([...files]);
    setLastSelectedIndex(files.length - 1);
  }, []);

  const selectFile = useCallback((file: File, files: File[], multi = false, range = false) => {
    const fileIndex = files.findIndex((f) => f.id === file.id);

    setLastSelectedIndex((prevLastIndex) => {
      if (range && prevLastIndex !== -1) {
        const start = Math.min(prevLastIndex, fileIndex);
        const end = Math.max(prevLastIndex, fileIndex);
        const rangeFiles = files.slice(start, end + 1);

        setSelectedFiles((prev) => {
          // If there's already a multi-file selection, add the range (Finder behavior)
          if (prev.length > 1) {
            // Create a map for O(1) lookup
            const existingIds = new Set(prev.map((f) => f.id));
            const combined = [...prev];

            // Add new range files that aren't already selected
            for (const rangeFile of rangeFiles) {
              if (!existingIds.has(rangeFile.id)) {
                combined.push(rangeFile);
              }
            }

            return combined;
          } else {
            // Single file or empty selection, replace with range
            return rangeFiles;
          }
        });
        setFocusedIndex(fileIndex);
        return fileIndex; // Update anchor to clicked file for next range
      } else if (multi) {
        setSelectedFiles((prev) => {
          const isSelected = prev.some((f) => f.id === file.id);
          if (isSelected) {
            return prev.filter((f) => f.id !== file.id);
          } else {
            return [...prev, file];
          }
        });
        setFocusedIndex(fileIndex);
        return fileIndex;
      } else {
        setSelectedFiles([file]);
        setFocusedIndex(fileIndex);
        return fileIndex;
      }
    });
  }, []);

  const moveFocus = useCallback((direction: "up" | "down" | "left" | "right", files: File[]) => {
    if (files.length === 0) return;

    setFocusedIndex((currentFocusedIndex) => {
      let newIndex = currentFocusedIndex;

      if (direction === "up") newIndex = Math.max(0, currentFocusedIndex - 1);
      if (direction === "down") newIndex = Math.min(files.length - 1, currentFocusedIndex + 1);
      if (direction === "left") newIndex = Math.max(0, currentFocusedIndex - 1);
      if (direction === "right") newIndex = Math.min(files.length - 1, currentFocusedIndex + 1);

      if (newIndex !== currentFocusedIndex) {
        setSelectedFiles([files[newIndex]]);
        setLastSelectedIndex(newIndex);
      }

      return newIndex;
    });
  }, []);

  // Create a Set of selected file IDs for O(1) lookup
  const selectedFileIds = useMemo(
    () => new Set(selectedFiles.map((f) => f.id)),
    [selectedFiles]
  );

  // Stable function for checking if a file is selected
  const isSelected = useCallback(
    (fileId: string) => selectedFileIds.has(fileId),
    [selectedFileIds]
  );

  const value = useMemo(() => ({
    selectedFiles,
    selectedFileIds,
    isSelected,
    setSelectedFiles,
    selectFile,
    clearSelection,
    selectAll,
    focusedIndex,
    setFocusedIndex,
    moveFocus,
  }), [
    selectedFiles,
    selectedFileIds,
    isSelected,
    selectFile,
    clearSelection,
    selectAll,
    focusedIndex,
    moveFocus,
  ]);

  return <SelectionContext.Provider value={value}>{children}</SelectionContext.Provider>;
}

export function useSelection() {
  const context = useContext(SelectionContext);
  if (!context) throw new Error("useSelection must be used within SelectionProvider");
  return context;
}
