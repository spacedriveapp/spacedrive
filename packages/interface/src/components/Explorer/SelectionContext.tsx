import { createContext, useContext, useState, useCallback, useMemo, useEffect, type ReactNode } from "react";
import { usePlatform } from "../../platform";
import type { File } from "@sd/ts-client/generated/types";

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

export function SelectionProvider({ children }: { children: ReactNode }) {
  const platform = usePlatform();
  const [selectedFiles, setSelectedFiles] = useState<File[]>([]);
  const [focusedIndex, setFocusedIndex] = useState(-1);
  const [lastSelectedIndex, setLastSelectedIndex] = useState(-1);

  // Sync selected file IDs to platform (for cross-window state sharing)
  useEffect(() => {
    const fileIds = selectedFiles.map((f) => f.id);

    if (platform.setSelectedFileIds) {
      platform.setSelectedFileIds(fileIds).catch((err) => {
        console.error("Failed to sync selected files to platform:", err);
      });
    }
  }, [selectedFiles, platform]);

  // Update native menu items based on selection
  useEffect(() => {
    const hasSelection = selectedFiles.length > 0;
    const isSingleSelection = selectedFiles.length === 1;

    platform.updateMenuItems?.([
      { id: "copy", enabled: hasSelection },
      { id: "cut", enabled: hasSelection },
      { id: "duplicate", enabled: hasSelection },
      { id: "rename", enabled: isSingleSelection },
      { id: "delete", enabled: hasSelection },
      { id: "paste", enabled: true },
    ]);
  }, [selectedFiles, platform]);

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

    if (range && lastSelectedIndex !== -1) {
      const start = Math.min(lastSelectedIndex, fileIndex);
      const end = Math.max(lastSelectedIndex, fileIndex);
      const rangeFiles = files.slice(start, end + 1);
      setSelectedFiles(rangeFiles);
      setFocusedIndex(fileIndex);
    } else if (multi) {
      const isSelected = selectedFiles.some((f) => f.id === file.id);
      if (isSelected) {
        setSelectedFiles(selectedFiles.filter((f) => f.id !== file.id));
      } else {
        setSelectedFiles([...selectedFiles, file]);
      }
      setLastSelectedIndex(fileIndex);
      setFocusedIndex(fileIndex);
    } else {
      setSelectedFiles([file]);
      setLastSelectedIndex(fileIndex);
      setFocusedIndex(fileIndex);
    }
  }, [selectedFiles, lastSelectedIndex]);

  const moveFocus = useCallback((direction: "up" | "down" | "left" | "right", files: File[]) => {
    if (files.length === 0) return;

    let newIndex = focusedIndex;

    if (direction === "up") newIndex = Math.max(0, focusedIndex - 1);
    if (direction === "down") newIndex = Math.min(files.length - 1, focusedIndex + 1);
    if (direction === "left") newIndex = Math.max(0, focusedIndex - 1);
    if (direction === "right") newIndex = Math.min(files.length - 1, focusedIndex + 1);

    if (newIndex !== focusedIndex) {
      setFocusedIndex(newIndex);
      setSelectedFiles([files[newIndex]]);
      setLastSelectedIndex(newIndex);
    }
  }, [focusedIndex]);

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

  const value = {
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
  };

  return <SelectionContext.Provider value={value}>{children}</SelectionContext.Provider>;
}

export function useSelection() {
  const context = useContext(SelectionContext);
  if (!context) throw new Error("useSelection must be used within SelectionProvider");
  return context;
}
