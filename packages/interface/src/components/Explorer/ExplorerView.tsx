import { useEffect } from "react";
import { useParams } from "react-router-dom";
import { useExplorer } from "./context";
import { useNormalizedCache } from "../../context";
import { useExplorerKeyboard } from "./hooks/useExplorerKeyboard";
import { GridView } from "./views/GridView";
import { ListView } from "./views/ListView";
import { MediaView } from "./views/MediaView";
import { ColumnView } from "./views/ColumnView";
import { SizeView } from "./views/SizeView";
import { EmptyView } from "./views/EmptyView";
import { TopBarPortal } from "../../TopBar";
import {
  SidebarSimple,
  Info,
  ArrowLeft,
  ArrowRight,
  Rows,
  GridFour,
  Camera,
  Columns,
  ChartPieSlice,
} from "@phosphor-icons/react";
import { TopBarButton, TopBarButtonGroup, SearchBar } from "@sd/ui";
import { PathBar } from "./components/PathBar";
import { ViewSettings } from "../Explorer/ViewSettings";
import { SortMenu } from "./SortMenu";

export function ExplorerView() {
  const { locationId } = useParams();

  // Enable keyboard navigation
  useExplorerKeyboard();
  const {
    sidebarVisible,
    setSidebarVisible,
    inspectorVisible,
    setInspectorVisible,
    viewMode,
    setViewMode,
    sortBy,
    setSortBy,
    goBack,
    goForward,
    canGoBack,
    canGoForward,
    currentPath,
    setCurrentPath,
    devices,
  } = useExplorer();

  // Fetch locations to get the SdPath for this locationId
  const locationsQuery = useNormalizedCache({
    wireMethod: "query:locations.list",
    input: null,
    resourceType: "location",
  });

  // Set currentPath from location ID (only when location changes)
  useEffect(() => {
    if (locationId && locationsQuery.data?.locations) {
      const location = locationsQuery.data.locations.find((loc: any) => loc.id === locationId);
      if (location?.sd_path) {
        // Only set if different to avoid infinite loops
        const currentPathStr = JSON.stringify(currentPath);
        const newPathStr = JSON.stringify(location.sd_path);

        if (currentPathStr !== newPathStr) {
          console.log("Setting currentPath from location:", location.sd_path);
          setCurrentPath(location.sd_path);
        }
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [locationId, locationsQuery.data]); // Don't include setCurrentPath or currentPath - causes infinite loop!

  if (!currentPath) {
    return <EmptyView />;
  }

  return (
    <>
      <TopBarPortal
        left={
          <div className="flex items-center gap-2">
            <TopBarButton
              icon={SidebarSimple}
              onClick={() => setSidebarVisible(!sidebarVisible)}
              active={sidebarVisible}
            />
            <TopBarButtonGroup>
              <TopBarButton
                icon={ArrowLeft}
                onClick={goBack}
                disabled={!canGoBack}
              />
              <TopBarButton
                icon={ArrowRight}
                onClick={goForward}
                disabled={!canGoForward}
              />
            </TopBarButtonGroup>
            {currentPath && (
              <PathBar
                path={currentPath}
                devices={devices}
                onNavigate={setCurrentPath}
              />
            )}
          </div>
        }
        right={
          <div className="flex items-center gap-2">
            <SearchBar className="w-64" placeholder="Search..." />
            <TopBarButtonGroup>
              <TopBarButton
                icon={Rows}
                active={viewMode === "list"}
                onClick={() => setViewMode("list")}
              />
              <TopBarButton
                icon={GridFour}
                active={viewMode === "grid"}
                onClick={() => setViewMode("grid")}
              />
              <TopBarButton
                icon={Columns}
                active={viewMode === "column"}
                onClick={() => setViewMode("column")}
              />
              <TopBarButton
                icon={Camera}
                active={viewMode === "media"}
                onClick={() => setViewMode("media")}
              />
              <TopBarButton
                icon={ChartPieSlice}
                active={viewMode === "size"}
                onClick={() => setViewMode("size")}
              />
            </TopBarButtonGroup>
            <ViewSettings />
            <SortMenu sortBy={sortBy} onSortChange={setSortBy} viewMode={viewMode} />
            <TopBarButton
              icon={Info}
              onClick={() => setInspectorVisible(!inspectorVisible)}
              active={inspectorVisible}
            />
          </div>
        }
      />

      <div className="relative flex w-full flex-col h-full overflow-hidden bg-app/80">
        <div className="flex-1 overflow-auto pt-[52px]">
          {viewMode === "grid" ? (
            <GridView />
          ) : viewMode === "list" ? (
            <ListView />
          ) : viewMode === "column" ? (
            <ColumnView />
          ) : viewMode === "size" ? (
            <SizeView />
          ) : (
            <MediaView />
          )}
        </div>
      </div>
    </>
  );
}
