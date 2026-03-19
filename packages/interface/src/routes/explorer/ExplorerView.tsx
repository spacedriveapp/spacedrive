import {
	ArrowLeft,
	ArrowRight,
	Info,
	SidebarSimple,
	Tag as TagIcon
} from '@phosphor-icons/react';
import {CircleButton, CircleButtonGroup} from '@spacedrive/primitives';
import clsx from 'clsx';
import {useCallback, useEffect, useMemo, useState} from 'react';
import {TopBarItem, TopBarPortal} from '../../TopBar';
import {ExpandableSearchButton} from './components/ExpandableSearchButton';
import {PathBar} from './components/PathBar';
import {VirtualPathBar} from './components/VirtualPathBar';
import {useExplorer, type ViewMode} from './context';
import {useVirtualListing} from './hooks/useVirtualListing';
import {SearchToolbar} from './SearchToolbar';
import {SortMenu, SortMenuPanel} from './SortMenu';
import {TabNavigationGuard} from './TabNavigationGuard';
import {ViewModeMenu, ViewModeMenuPanel} from './ViewModeMenu';
import {ColumnView} from './views/ColumnView';
import {EmptyView} from './views/EmptyView';
import {GridView} from './views/GridView';
import {KnowledgeView} from './views/KnowledgeView';
import {ListView} from './views/ListView';
import {MediaView} from './views/MediaView';
import {SearchView} from './views/SearchView';
import {SizeView} from './views/SizeView';
import {ViewSettings, ViewSettingsPanel} from './ViewSettings';

export function ExplorerView() {
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
		currentFiles,
		columnStack
	} = useExplorer();

	const {isVirtualView} = useVirtualListing();
	const isPreviewActive = !!quickPreviewFileId;

	// In column view, the path bar should reflect the deepest column, not the root
	const pathBarPath = useMemo(() => {
		if (viewMode === 'column' && columnStack.length > 1) {
			return columnStack[columnStack.length - 1];
		}
		return currentPath;
	}, [viewMode, columnStack, currentPath]);

	const [searchValue, setSearchValue] = useState('');

	const handleSearchChange = useCallback(
		(value: string) => {
			setSearchValue(value);

			if (value.length >= 2) {
				const timeoutId = setTimeout(() => {
					enterSearchMode(value);
				}, 300);
				return () => clearTimeout(timeoutId);
			} else if (value.length === 0 && mode.type === 'search') {
				exitSearchMode();
			}
		},
		[enterSearchMode, exitSearchMode, mode.type]
	);

	const handleSearchClear = useCallback(() => {
		setSearchValue('');
		exitSearchMode();
	}, [exitSearchMode]);

	useEffect(() => {
		if (mode.type !== 'search') {
			setSearchValue('');
		}
	}, [mode.type]);

	// When leaving column view, navigate to the deepest column so the
	// new view shows the directory the user was actually looking at.
	const handleViewModeChange = useCallback(
		(newMode: string) => {
			if (
				viewMode === 'column' &&
				newMode !== 'column' &&
				columnStack.length > 1
			) {
				navigateToPath(columnStack[columnStack.length - 1]);
			}
			setViewMode(newMode as ViewMode);
		},
		[viewMode, columnStack, navigateToPath, setViewMode]
	);

	// Memoize submenu content to prevent infinite re-renders
	const viewModeSubmenu = useMemo(
		() => (
			<ViewModeMenuPanel
				viewMode={viewMode}
				onViewModeChange={handleViewModeChange}
			/>
		),
		[viewMode, handleViewModeChange]
	);

	const viewSettingsSubmenu = useMemo(
		() => (
			<ViewSettingsPanel
				viewSettings={viewSettings}
				setViewSettings={setViewSettings}
				viewMode={viewMode}
				totalFileCount={currentFiles.length}
			/>
		),
		[viewSettings, setViewSettings, viewMode, currentFiles.length]
	);

	const sortSubmenu = useMemo(
		() => (
			<SortMenuPanel
				sortBy={sortBy}
				onSortChange={setSortBy}
				viewMode={viewMode as any}
			/>
		),
		[sortBy, setSortBy, viewMode]
	);

	// Allow rendering if we have a currentPath, a virtual view, or a special mode (tag/recents)
	if (!currentPath && !isVirtualView && mode.type === "browse") {
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
								priority="normal"
								onClick={() =>
									setSidebarVisible(!sidebarVisible)
								}
							>
								<CircleButton
									icon={SidebarSimple}
									onClick={() =>
										setSidebarVisible(!sidebarVisible)
									}
									active={!sidebarVisible}
								/>
							</TopBarItem>
							<TopBarItem
								id="navigation"
								label="Navigation"
								priority="high"
							>
								<CircleButtonGroup>
									<CircleButton
										icon={ArrowLeft}
										onClick={goBack}
										disabled={!canGoBack}
									/>
									<CircleButton
										icon={ArrowRight}
										onClick={goForward}
										disabled={!canGoForward}
									/>
								</CircleButtonGroup>
							</TopBarItem>
							{pathBarPath && (
								<TopBarItem
									id="path-bar"
									label="Path"
									priority="high"
								>
									<PathBar
										path={pathBarPath}
										devices={devices}
										onNavigate={navigateToPath}
									/>
								</TopBarItem>
							)}
							{currentView && (
								<TopBarItem
									id="virtual-path-bar"
									label="Path"
									priority="high"
								>
									<VirtualPathBar
										view={currentView}
										devices={devices}
									/>
								</TopBarItem>
							)}
						</>
					}
					right={
						<>
							<TopBarItem
								id="search"
								label="Search"
								priority="high"
							>
								<ExpandableSearchButton
									placeholder={
										currentPath
											? 'Search in current folder...'
											: 'Search...'
									}
									value={searchValue}
									onChange={handleSearchChange}
									onClear={handleSearchClear}
								/>
							</TopBarItem>
							<TopBarItem
								id="tag-mode"
								label="Tags"
								priority="low"
								onClick={() => setTagModeActive(!tagModeActive)}
							>
								<CircleButton
									icon={TagIcon}
									onClick={() =>
										setTagModeActive(!tagModeActive)
									}
									active={tagModeActive}
								/>
							</TopBarItem>
							<TopBarItem
								id="view-mode"
								label="Views"
								priority="normal"
								submenuContent={viewModeSubmenu}
							>
								<ViewModeMenu
									viewMode={viewMode}
									onViewModeChange={handleViewModeChange}
								/>
							</TopBarItem>
							<TopBarItem
								id="view-settings"
								label="View Settings"
								priority="low"
								submenuContent={viewSettingsSubmenu}
							>
								<ViewSettings
									totalFileCount={currentFiles.length}
								/>
							</TopBarItem>
							<TopBarItem
								id="sort"
								label="Sort"
								priority="low"
								submenuContent={sortSubmenu}
							>
								<SortMenu
									sortBy={sortBy}
									onSortChange={setSortBy}
									viewMode={viewMode as any}
								/>
							</TopBarItem>
							<TopBarItem
								id="inspector-toggle"
								label="Inspector"
								priority="high"
								onClick={() =>
									setInspectorVisible(!inspectorVisible)
								}
							>
								<CircleButton
									icon={Info}
									onClick={() =>
										setInspectorVisible(!inspectorVisible)
									}
									active={!inspectorVisible}
								/>
							</TopBarItem>
						</>
					}
				/>
			)}

			<div
				className={clsx(
					'relative flex h-full w-full flex-col overflow-hidden pt-1.5',
					viewMode === 'size' ? 'bg-transparent' : 'bg-app/80'
				)}
			>
				{mode.type === 'search' && <SearchToolbar />}
				<div
					className={clsx(
						'flex-1',
						viewMode === 'size'
							? 'overflow-visible'
							: 'overflow-auto'
					)}
				>
					<TabNavigationGuard>
						{mode.type === 'search' ? (
							<SearchView />
						) : viewMode === 'grid' ? (
							<GridView />
						) : viewMode === 'list' ? (
							<ListView />
						) : viewMode === 'column' ? (
							<ColumnView />
						) : viewMode === 'size' ? (
							<SizeView />
						) : viewMode === 'knowledge' ? (
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
