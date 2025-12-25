import { useExplorer } from "./context";
import { GridView } from "./views/GridView";
import { ListView } from "./views/ListView";
import { MediaView } from "./views/MediaView";
import { ColumnView } from "./views/ColumnView";
import { SizeView } from "./views/SizeView";
import { KnowledgeView } from "./views/KnowledgeView";
import { EmptyView } from "./views/EmptyView";
import { SearchView } from "./views/SearchView";
import { SearchToolbar } from "./SearchToolbar";
import { TopBarPortal } from "../../TopBar";
import { useVirtualListing } from "./hooks/useVirtualListing";
import { VirtualPathBar } from "./components/VirtualPathBar";
import {
	SidebarSimple,
	Info,
	ArrowLeft,
	ArrowRight,
	Tag as TagIcon,
} from "@phosphor-icons/react";
import { TopBarButton, TopBarButtonGroup, SearchBar } from "@sd/ui";
import { PathBar } from "./components/PathBar";
import { ViewSettings } from "../Explorer/ViewSettings";
import { SortMenu } from "./SortMenu";
import { ViewModeMenu } from "./ViewModeMenu";
import { TabNavigationGuard } from "./TabNavigationGuard";
import { useState, useEffect, useCallback } from "react";

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
			} else if (value.length === 0 && mode.type === "search") {
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

	// Allow rendering if either we have a currentPath or we're in a virtual view
	if (!currentPath && !isVirtualView) {
		return <EmptyView />;
	}

	return (
		<>
			{!isPreviewActive && (
				<TopBarPortal
					left={
						<div className="flex items-center gap-2">
							<TopBarButton
								icon={SidebarSimple}
								onClick={() =>
									setSidebarVisible(!sidebarVisible)
								}
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
									onNavigate={navigateToPath}
								/>
							)}
							{currentView && (
								<VirtualPathBar
									view={currentView}
									devices={devices}
								/>
							)}
						</div>
					}
					right={
						<div className="flex items-center gap-2">
							<SearchBar
								className="w-64"
								placeholder={
									currentPath
										? "Search in current folder..."
										: "Search..."
								}
								value={searchValue}
								onChange={handleSearchChange}
								onClear={handleSearchClear}
							/>
							<TopBarButton
								icon={TagIcon}
								onClick={() => setTagModeActive(!tagModeActive)}
								active={tagModeActive}
							/>
							<ViewModeMenu
								viewMode={viewMode}
								onViewModeChange={setViewMode}
							/>
							<ViewSettings />
							<SortMenu
								sortBy={sortBy}
								onSortChange={setSortBy}
								viewMode={viewMode as any}
							/>
							<TopBarButton
								icon={Info}
								onClick={() =>
									setInspectorVisible(!inspectorVisible)
								}
								active={inspectorVisible}
							/>
						</div>
					}
				/>
			)}

			<div className="relative flex w-full flex-col pt-1.5 h-full overflow-hidden bg-app/80">
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
