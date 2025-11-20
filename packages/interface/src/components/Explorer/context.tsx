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
  openQuickPreview: (fileId: string) => void;
  closeQuickPreview: () => void;
  goToNextPreview: (files: File[]) => void;
  goToPreviousPreview: (files: File[]) => void;

  devices: Map<string, LibraryDeviceInfo>;
}

const ExplorerContext = createContext<ExplorerState | null>(null);

export function ExplorerProvider({ children }: { children: ReactNode }) {
  const platform = usePlatform();
  const [currentPath, setCurrentPathInternal] = useState<SdPath | null>(null);
  const [history, setHistory] = useState<SdPath[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
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

  const setViewSettings = (settings: Partial<ViewSettings>) => {
    setViewSettingsInternal((prev) => ({ ...prev, ...settings }));
  };

  const devicesQuery = useLibraryQuery({
    type: "devices.list",
    input: { include_offline: true, include_details: false },
  });

  const devices = useMemo(() => {
    const deviceList = devicesQuery.data || [];
    return new Map(deviceList.map((d) => [d.id, d]));
  }, [devicesQuery.data]);


  const goBack = () => {
    if (historyIndex > 0) {
      const newIndex = historyIndex - 1;
      setHistoryIndex(newIndex);
      setCurrentPathInternal(history[newIndex]);
    }
  };

  const goForward = () => {
    if (historyIndex < history.length - 1) {
      const newIndex = historyIndex + 1;
      setHistoryIndex(newIndex);
      setCurrentPathInternal(history[newIndex]);
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
  };

  const openQuickPreview = (fileId: string) => {
    setQuickPreviewFileId(fileId);
  };

  const closeQuickPreview = () => {
    setQuickPreviewFileId(null);
  };

  const goToNextPreview = (files: File[]) => {
    if (!quickPreviewFileId) return;
    const currentIndex = files.findIndex(f => f.id === quickPreviewFileId);
    if (currentIndex < files.length - 1) {
      setQuickPreviewFileId(files[currentIndex + 1].id);
    }
  };

  const goToPreviousPreview = (files: File[]) => {
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
