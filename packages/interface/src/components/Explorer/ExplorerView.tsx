import { useEffect } from "react";
import { useSearchParams } from "react-router-dom";
import { useExplorer } from "./context";
import { GridView } from "./views/GridView";
import { ListView } from "./views/ListView";
import { MediaView } from "./views/MediaView";
import { ColumnView } from "./views/ColumnView";
import { SizeView } from "./views/SizeView";
import { KnowledgeView } from "./views/KnowledgeView";
import { EmptyView } from "./views/EmptyView";
import { TopBarPortal } from "../../TopBar";
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

export function ExplorerView() {
	const [searchParams] = useSearchParams();
	const {
		sidebarVisible,
		setSidebarVisible,
		inspectorVisible,
		setInspectorVisible,
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
		setCurrentPath,
		syncPathFromUrl,
		devices,
		quickPreviewFileId,
	} = useExplorer();

	const isPreviewActive = !!quickPreviewFileId;

	// Sync currentPath from URL query parameter
	useEffect(() => {
		const pathParam = searchParams.get("path");
		if (pathParam) {
			try {
				const sdPath = JSON.parse(decodeURIComponent(pathParam));
				const currentPathStr = JSON.stringify(currentPath);
				const newPathStr = JSON.stringify(sdPath);

				if (currentPathStr !== newPathStr) {
					syncPathFromUrl(sdPath);
				}
			} catch (e) {
				console.error("Failed to parse path query parameter:", e);
			}
		}
	}, [searchParams, currentPath, syncPathFromUrl]);

	if (!currentPath) {
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
									onNavigate={setCurrentPath}
								/>
							)}
						</div>
					}
					right={
						<div className="flex items-center gap-2">
							<SearchBar
								className="w-64"
								placeholder="Search..."
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
					) : viewMode === "knowledge" ? (
						<KnowledgeView />
					) : (
						<MediaView />
					)}
				</div>
			</div>
		</>
	);
}
