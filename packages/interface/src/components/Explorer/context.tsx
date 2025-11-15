import {
  createContext,
  useContext,
  useState,
  useMemo,
  useEffect,
  type ReactNode,
} from "react";
import { useLibraryQuery, useNormalizedCache } from "../../context";
import { usePlatform } from "../../platform";

import type {
  SdPath,
  File,
  LibraryDeviceInfo,
  DirectorySortBy,
  MediaSortBy,
} from "@sd/ts-client/generated/types";

interface ViewSettings {
  gridSize: number; // 80-400px
  gapSize: number; // 1-32px
  showFileSize: boolean;
  columnWidth: number; // 200-400px for column view
}

interface ExplorerState {
  currentPath: SdPath | null;
  setCurrentPath: (path: SdPath | null) => void;

  history: SdPath[];
  historyIndex: number;
  goBack: () => void;
  goForward: () => void;
  canGoBack: boolean;
  canGoForward: boolean;

  selectedFiles: File[];
  setSelectedFiles: (files: File[]) => void;
  selectFile: (file: File, multi?: boolean, range?: boolean) => void;
  clearSelection: () => void;
  selectAll: () => void;

  focusedIndex: number;
  setFocusedIndex: (index: number) => void;
  moveFocus: (direction: "up" | "down" | "left" | "right") => void;

  viewMode: "grid" | "list" | "media" | "column" | "size";
  setViewMode: (mode: "grid" | "list" | "media" | "column" | "size") => void;

  sortBy: DirectorySortBy | MediaSortBy;
  setSortBy: (sort: DirectorySortBy | MediaSortBy) => void;

  viewSettings: ViewSettings;
  setViewSettings: (settings: Partial<ViewSettings>) => void;

  sidebarVisible: boolean;
  setSidebarVisible: (visible: boolean) => void;
  inspectorVisible: boolean;
  setInspectorVisible: (visible: boolean) => void;

  quickPreviewFileId: string | null;
  setQuickPreviewFileId: (fileId: string | null) => void;
  openQuickPreview: () => void;
  closeQuickPreview: () => void;
  goToNextPreview: () => void;
  goToPreviousPreview: () => void;

  files: File[];
  isLoading: boolean;
  error: Error | null;

  devices: Map<string, LibraryDeviceInfo>;
}

const ExplorerContext = createContext<ExplorerState | null>(null);

export function ExplorerProvider({ children }: { children: ReactNode }) {
  const platform = usePlatform();
  const [currentPath, setCurrentPathInternal] = useState<SdPath | null>(null);
  const [history, setHistory] = useState<SdPath[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const [selectedFiles, setSelectedFiles] = useState<File[]>([]);
  const [focusedIndex, setFocusedIndex] = useState(-1);
  const [lastSelectedIndex, setLastSelectedIndex] = useState(-1);
  const [viewMode, setViewMode] = useState<"grid" | "list" | "media" | "column" | "size">("grid");
  const [sortBy, setSortBy] = useState<DirectorySortBy | MediaSortBy>("name");

  // Update sort when switching to media view
  useEffect(() => {
    if (viewMode === "media" && sortBy === "type") {
      // "type" is not available in media view, switch to date taken
      setSortBy("datetaken");
    } else if (viewMode !== "media" && sortBy === "datetaken") {
      // "datetaken" is not available outside media view, switch to modified
      setSortBy("modified");
    }
  }, [viewMode, sortBy]);
  const [viewSettings, setViewSettingsInternal] = useState<ViewSettings>({
    gridSize: 120,
    gapSize: 16,
    showFileSize: true,
    columnWidth: 256,
  });
  const [sidebarVisible, setSidebarVisible] = useState(true);
  const [inspectorVisible, setInspectorVisible] = useState(true);
  const [quickPreviewFileId, setQuickPreviewFileId] = useState<string | null>(null);

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
      // Paste is always available (depends on clipboard, not selection)
      { id: "paste", enabled: true },
    ]);
  }, [selectedFiles, platform]);

  const setViewSettings = (settings: Partial<ViewSettings>) => {
    setViewSettingsInternal((prev) => ({ ...prev, ...settings }));
  };

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
    // Path-scoped filtering: backend now includes affected_paths in metadata
    // and filters events efficiently using parent directory matching
    pathScope: currentPath ?? undefined,
  });

  const devicesQuery = useLibraryQuery({
    type: "devices.list",
    input: { include_offline: true, include_details: false },
  });

  const files = directoryQuery.data?.files || [];

  // Initialize focused index when files load
  useEffect(() => {
    if (files.length > 0 && focusedIndex === -1) {
      setFocusedIndex(0);
    }
  }, [files, focusedIndex]);

  const devices = useMemo(() => {
    const deviceList = devicesQuery.data || [];
    return new Map(deviceList.map((d) => [d.id, d]));
  }, [devicesQuery.data]);

  const clearSelection = () => {
    setSelectedFiles([]);
    setFocusedIndex(-1);
    setLastSelectedIndex(-1);
  };

  const selectAll = () => {
    setSelectedFiles([...files]);
    setLastSelectedIndex(files.length - 1);
  };

  const moveFocus = (direction: "up" | "down" | "left" | "right") => {
    if (files.length === 0) return;

    let newIndex = focusedIndex;

    if (viewMode === "list" || viewMode === "column") {
      if (direction === "up") newIndex = Math.max(0, focusedIndex - 1);
      if (direction === "down")
        newIndex = Math.min(files.length - 1, focusedIndex + 1);
      // For column view, left/right will be handled separately for column navigation
    } else {
      const containerWidth =
        window.innerWidth -
        (sidebarVisible ? 224 : 0) -
        (inspectorVisible ? 284 : 0) -
        48;
      const itemWidth = viewSettings.gridSize + viewSettings.gapSize;
      const columns = Math.floor(containerWidth / itemWidth);

      if (direction === "up") newIndex = Math.max(0, focusedIndex - columns);
      if (direction === "down")
        newIndex = Math.min(files.length - 1, focusedIndex + columns);
      if (direction === "left") newIndex = Math.max(0, focusedIndex - 1);
      if (direction === "right")
        newIndex = Math.min(files.length - 1, focusedIndex + 1);
    }

    if (newIndex !== focusedIndex) {
      setFocusedIndex(newIndex);
      setSelectedFiles([files[newIndex]]);
      setLastSelectedIndex(newIndex);
    }
  };

  // Keyboard event handling
  useEffect(() => {
    const handleKeyDown = async (e: KeyboardEvent) => {
      // Arrow keys: Navigation
      if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.key)) {
        e.preventDefault();
        const direction = e.key.replace("Arrow", "").toLowerCase() as
          | "up"
          | "down"
          | "left"
          | "right";
        moveFocus(direction);
        return;
      }

      // Cmd/Ctrl+A: Select all
      if ((e.metaKey || e.ctrlKey) && e.key === "a") {
        e.preventDefault();
        selectAll();
        return;
      }

      // Spacebar: Open Quick Preview (in-app modal)
      if (e.code === "Space" && selectedFiles.length === 1) {
        e.preventDefault();
        setQuickPreviewFileId(selectedFiles[0].id);
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
    focusedIndex,
    files,
    viewMode,
    viewSettings,
    sidebarVisible,
    inspectorVisible,
  ]);

  const goBack = () => {
    if (historyIndex > 0) {
      const newIndex = historyIndex - 1;
      setHistoryIndex(newIndex);
      setCurrentPathInternal(history[newIndex]);
      setSelectedFiles([]);
    }
  };

  const goForward = () => {
    if (historyIndex < history.length - 1) {
      const newIndex = historyIndex + 1;
      setHistoryIndex(newIndex);
      setCurrentPathInternal(history[newIndex]);
      setSelectedFiles([]);
    }
  };

  const canGoBack = historyIndex > 0;
  const canGoForward = historyIndex < history.length - 1;

  const setCurrentPath = (path: SdPath | null) => {
    if (path) {
      const newHistory = history.slice(0, historyIndex + 1);
      newHistory.push(path);
      setHistory(newHistory);
      setHistoryIndex(newHistory.length - 1);
    }
    setCurrentPathInternal(path);
    setSelectedFiles([]);
    setFocusedIndex(-1);
    setLastSelectedIndex(-1);
  };

  const selectFile = (file: File, multi = false, range = false) => {
    const fileIndex = files.findIndex((f) => f.id === file.id);

    console.log("selectFile called:", {
      fileName: file.name,
      fileId: file.id,
      multi,
      range,
      fileIndex,
      currentSelected: selectedFiles.length,
    });

    if (range && lastSelectedIndex !== -1) {
      // Shift+Click: Select range
      const start = Math.min(lastSelectedIndex, fileIndex);
      const end = Math.max(lastSelectedIndex, fileIndex);
      const rangeFiles = files.slice(start, end + 1);
      setSelectedFiles(rangeFiles);
      setFocusedIndex(fileIndex);
    } else if (multi) {
      // Cmd/Ctrl+Click: Toggle selection
      const isSelected = selectedFiles.some((f) => f.id === file.id);
      if (isSelected) {
        setSelectedFiles(selectedFiles.filter((f) => f.id !== file.id));
      } else {
        setSelectedFiles([...selectedFiles, file]);
      }
      setLastSelectedIndex(fileIndex);
      setFocusedIndex(fileIndex);
    } else {
      // Normal click: Select single
      setSelectedFiles([file]);
      setLastSelectedIndex(fileIndex);
      setFocusedIndex(fileIndex);
    }
  };

  const openQuickPreview = () => {
    if (selectedFiles.length === 1) {
      setQuickPreviewFileId(selectedFiles[0].id);
    }
  };

  const closeQuickPreview = () => {
    setQuickPreviewFileId(null);
  };

  const goToNextPreview = () => {
    if (!quickPreviewFileId) return;
    const currentIndex = files.findIndex(f => f.id === quickPreviewFileId);
    if (currentIndex < files.length - 1) {
      setQuickPreviewFileId(files[currentIndex + 1].id);
    }
  };

  const goToPreviousPreview = () => {
    if (!quickPreviewFileId) return;
    const currentIndex = files.findIndex(f => f.id === quickPreviewFileId);
    if (currentIndex > 0) {
      setQuickPreviewFileId(files[currentIndex - 1].id);
    }
  };

  const value: ExplorerState = {
    currentPath,
    setCurrentPath,
    history,
    historyIndex,
    goBack,
    goForward,
    canGoBack,
    canGoForward,
    selectedFiles,
    setSelectedFiles,
    selectFile,
    clearSelection,
    selectAll,
    focusedIndex,
    setFocusedIndex,
    moveFocus,
    viewMode,
    setViewMode,
    sortBy,
    setSortBy,
    viewSettings,
    setViewSettings,
    sidebarVisible,
    setSidebarVisible,
    inspectorVisible,
    setInspectorVisible,
    quickPreviewFileId,
    setQuickPreviewFileId,
    openQuickPreview,
    closeQuickPreview,
    goToNextPreview,
    goToPreviousPreview,
    files,
    isLoading: directoryQuery.isLoading,
    error: directoryQuery.error as Error | null,
    devices,
  };

  return (
    <ExplorerContext.Provider value={value}>
      {children}
    </ExplorerContext.Provider>
  );
}

export function useExplorer() {
  const context = useContext(ExplorerContext);
  if (!context)
    throw new Error("useExplorer must be used within ExplorerProvider");
  return context;
}
