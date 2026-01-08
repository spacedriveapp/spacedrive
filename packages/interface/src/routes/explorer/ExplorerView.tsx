import {
  ArrowLeft,
  ArrowRight,
  Info,
  SidebarSimple,
  Tag as TagIcon,
} from "@phosphor-icons/react";
import { TopBarButton, TopBarButtonGroup } from "@sd/ui";
import { useCallback, useEffect, useMemo, useState } from "react";
import { TopBarItem, TopBarPortal } from "../../TopBar";
import { ExpandableSearchButton } from "./components/ExpandableSearchButton";
import { PathBar } from "./components/PathBar";
import { VirtualPathBar } from "./components/VirtualPathBar";
import { useExplorer } from "./context";
import { useVirtualListing } from "./hooks/useVirtualListing";
import { SearchToolbar } from "./SearchToolbar";
import { SortMenu, SortMenuPanel } from "./SortMenu";
import { TabNavigationGuard } from "./TabNavigationGuard";
import { ViewModeMenu, ViewModeMenuPanel } from "./ViewModeMenu";
import { ViewSettings, ViewSettingsPanel } from "./ViewSettings";
import { ColumnView } from "./views/ColumnView";
import { EmptyView } from "./views/EmptyView";
import { GridView } from "./views/GridView";
import { KnowledgeView } from "./views/KnowledgeView";
import { ListView } from "./views/ListView";
import { MediaView } from "./views/MediaView";
import { SearchView } from "./views/SearchView";
import { SizeView } from "./views/SizeView";

export function ExplorerView() {
  const {
    sidebarVisible,
    setSidebarVisible,
    inspectorVisible,
    setInspectorVisible,
    activeTabId,
    tagModeActive,
    setTagModeActive,
    viewMode,
    setViewMode,
    sortBy,
    setSortBy,
    viewSettings,
    setViewSettings,
    goBack,
    goForward,
    canGoBack,
    canGoForward,
    currentPath,
    currentView,
    navigateToPath,
    devices,
    quickPreviewFileId,
    mode,
    enterSearchMode,
    exitSearchMode,
  } = useExplorer();

  const { isVirtualView } = useVirtualListing();
  const isPreviewActive = !!quickPreviewFileId;

  const [searchValue, setSearchValue] = useState("");

  const handleSearchChange = useCallback(
    (value: string) => {
      setSearchValue(value);

      if (value.length >= 2) {
        const timeoutId = setTimeout(() => {
          enterSearchMode(value);
        }, 300);
        return () => clearTimeout(timeoutId);
      }
      if (value.length === 0 && mode.type === "search") {
        exitSearchMode();
      }
    },
    [enterSearchMode, exitSearchMode, mode.type]
  );

  const handleSearchClear = useCallback(() => {
    setSearchValue("");
    exitSearchMode();
  }, [exitSearchMode]);

  useEffect(() => {
    if (mode.type !== "search") {
      setSearchValue("");
    }
  }, [mode.type]);

  // Memoize submenu content to prevent infinite re-renders
  const viewModeSubmenu = useMemo(
    () => (
      <ViewModeMenuPanel onViewModeChange={setViewMode} viewMode={viewMode} />
    ),
    [viewMode, setViewMode]
  );

  const viewSettingsSubmenu = useMemo(
    () => (
      <ViewSettingsPanel
        setViewSettings={setViewSettings}
        viewMode={viewMode}
        viewSettings={viewSettings}
      />
    ),
    [viewSettings, setViewSettings, viewMode]
  );

  const sortSubmenu = useMemo(
    () => (
      <SortMenuPanel
        onSortChange={setSortBy}
        sortBy={sortBy}
        viewMode={viewMode as any}
      />
    ),
    [sortBy, setSortBy, viewMode]
  );

  // Allow rendering if either we have a currentPath or we're in a virtual view
  if (!(currentPath || isVirtualView)) {
    return <EmptyView />;
  }

  return (
    <>
      {!isPreviewActive && (
        <TopBarPortal
          left={
            <>
              <TopBarItem
                id="sidebar-toggle"
                label="Sidebar"
                onClick={() => setSidebarVisible(!sidebarVisible)}
                priority="normal"
              >
                <TopBarButton
                  active={sidebarVisible}
                  icon={SidebarSimple}
                  onClick={() => setSidebarVisible(!sidebarVisible)}
                />
              </TopBarItem>
              <TopBarItem id="navigation" label="Navigation" priority="high">
                <TopBarButtonGroup>
                  <TopBarButton
                    disabled={!canGoBack}
                    icon={ArrowLeft}
                    onClick={goBack}
                  />
                  <TopBarButton
                    disabled={!canGoForward}
                    icon={ArrowRight}
                    onClick={goForward}
                  />
                </TopBarButtonGroup>
              </TopBarItem>
              {currentPath && (
                <TopBarItem id="path-bar" label="Path" priority="high">
                  <PathBar
                    devices={devices}
                    onNavigate={navigateToPath}
                    path={currentPath}
                  />
                </TopBarItem>
              )}
              {currentView && (
                <TopBarItem id="virtual-path-bar" label="Path" priority="high">
                  <VirtualPathBar devices={devices} view={currentView} />
                </TopBarItem>
              )}
            </>
          }
          right={
            <>
              <TopBarItem id="search" label="Search" priority="high">
                <ExpandableSearchButton
                  onChange={handleSearchChange}
                  onClear={handleSearchClear}
                  placeholder={
                    currentPath ? "Search in current folder..." : "Search..."
                  }
                  value={searchValue}
                />
              </TopBarItem>
              <TopBarItem
                id="tag-mode"
                label="Tags"
                onClick={() => setTagModeActive(!tagModeActive)}
                priority="low"
              >
                <TopBarButton
                  active={tagModeActive}
                  icon={TagIcon}
                  onClick={() => setTagModeActive(!tagModeActive)}
                />
              </TopBarItem>
              <TopBarItem
                id="view-mode"
                label="Views"
                priority="normal"
                submenuContent={viewModeSubmenu}
              >
                <ViewModeMenu
                  onViewModeChange={setViewMode}
                  viewMode={viewMode}
                />
              </TopBarItem>
              <TopBarItem
                id="view-settings"
                label="View Settings"
                priority="low"
                submenuContent={viewSettingsSubmenu}
              >
                <ViewSettings />
              </TopBarItem>
              <TopBarItem
                id="sort"
                label="Sort"
                priority="low"
                submenuContent={sortSubmenu}
              >
                <SortMenu
                  onSortChange={setSortBy}
                  sortBy={sortBy}
                  viewMode={viewMode as any}
                />
              </TopBarItem>
              <TopBarItem
                id="inspector-toggle"
                label="Inspector"
                onClick={() => setInspectorVisible(!inspectorVisible)}
                priority="high"
              >
                <TopBarButton
                  active={inspectorVisible}
                  icon={Info}
                  onClick={() => setInspectorVisible(!inspectorVisible)}
                />
              </TopBarItem>
            </>
          }
        />
      )}

      <div className="relative flex h-full w-full flex-col overflow-hidden bg-app/80 pt-1.5">
        {mode.type === "search" && <SearchToolbar />}
        <div className="flex-1 overflow-auto">
          <TabNavigationGuard>
            {mode.type === "search" ? (
              <SearchView />
            ) : viewMode === "grid" ? (
              <GridView />
            ) : viewMode === "list" ? (
              <ListView />
            ) : viewMode === "column" ? (
              <ColumnView />
            ) : viewMode === "size" ? (
              <SizeView />
            ) : viewMode === "knowledge" ? (
              <KnowledgeView />
            ) : (
              <MediaView />
            )}
          </TabNavigationGuard>
        </div>
      </div>
    </>
  );
}
