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
		currentView,
		setCurrentPath,
		syncPathFromUrl,
		syncViewFromUrl,
		devices,
		quickPreviewFileId,
	} = useExplorer();

	const { isVirtualView } = useVirtualListing();
	const isPreviewActive = !!quickPreviewFileId;

	// Sync currentPath or currentView from URL query parameters
	useEffect(() => {
		const pathParam = searchParams.get("path");
		const viewParam = searchParams.get("view");

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
		} else if (viewParam) {
			const id = searchParams.get("id");
			const params: Record<string, string> = {};
			searchParams.forEach((value, key) => {
				if (key !== "view" && key !== "id") {
					params[key] = value;
				}
			});

			const newView = {
				view: viewParam,
				id: id || undefined,
				params: Object.keys(params).length > 0 ? params : undefined,
			};
			const currentViewStr = JSON.stringify(currentView);
			const newViewStr = JSON.stringify(newView);

			if (currentViewStr !== newViewStr) {
				syncViewFromUrl(newView);
			}
		}
	}, [
		searchParams,
		currentPath,
		currentView,
		syncPathFromUrl,
		syncViewFromUrl,
	]);

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
									onNavigate={setCurrentPath}
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
