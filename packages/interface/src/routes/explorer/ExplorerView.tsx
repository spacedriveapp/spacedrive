import {
	ArrowLeft,
	ArrowRight,
	Info,
	SidebarSimple,
	Tag as TagIcon
} from '@phosphor-icons/react';
import {CircleButton, CircleButtonGroup} from '@spacedrive/primitives';
import clsx from 'clsx';
import {
	useCallback,
	useEffect,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
	type KeyboardEvent
} from 'react';
import {useLocation} from 'react-router-dom';
import {TopBarItem, TopBarPortal} from '../../TopBar';
import {
	ExpandableSearchField,
	type ExpandableSearchFieldHandle
} from './components/ExpandableSearchButton';
import {PathBar} from './components/PathBar';
import {PlanPreviewBanner} from './components/PlanPreviewBanner';
import {VirtualPathBar} from './components/VirtualPathBar';
import {
	getSpaceItemKeyFromRoute,
	MIN_SEARCH_QUERY_LENGTH,
	searchFiltersNarrow,
	useExplorer,
	type ViewMode
} from './context';
import {useKeybind} from '../../hooks/useKeybind';
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

export function ExplorerView({
	defaultViewMode = 'grid'
}: {
	defaultViewMode?: ViewMode;
}) {
	const location = useLocation();
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
		searchBar,
		searchFilters,
		openSearch,
		pinSearch,
		setSearchQuery,
		exitSearchMode,
		currentFiles,
		columnStack,
		loadPreferencesForSpaceItem
	} = useExplorer();

	useLayoutEffect(() => {
		loadPreferencesForSpaceItem(
			getSpaceItemKeyFromRoute(location.pathname, location.search),
			defaultViewMode
		);
	}, [
		defaultViewMode,
		loadPreferencesForSpaceItem,
		location.pathname,
		location.search
	]);

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

	// One search per settled query. The effect owns the timer, so every
	// keystroke cancels the previous one. Dropping under the minimum length
	// clears the query at once, including the two-to-one-character
	// backspace, which ends the search unless a filter carries it.
	useEffect(() => {
		if (searchValue.length < MIN_SEARCH_QUERY_LENGTH) {
			setSearchQuery('');
			return;
		}
		const timeoutId = setTimeout(() => setSearchQuery(searchValue), 300);
		return () => clearTimeout(timeoutId);
	}, [searchValue, setSearchQuery]);

	const handleSearchChange = useCallback((value: string) => {
		setSearchValue(value);
	}, []);

	// A closed search forgets its text however it closed: Escape, the close
	// button, a click away, or navigation.
	useEffect(() => {
		if (searchBar === 'closed') {
			setSearchValue('');
		}
	}, [searchBar]);

	const searchInputRef = useRef<ExpandableSearchFieldHandle>(null);
	useKeybind(
		'global.focusSearchBar',
		() => {
			openSearch();
			searchInputRef.current?.focus();
		},
		{ignoreWhenInputFocused: false}
	);

	const searchFieldRef = useRef<HTMLDivElement>(null);
	const searchBarRef = useRef<HTMLDivElement>(null);
	const searchHeld = searchValue !== '' || searchFiltersNarrow(searchFilters);

	// An open search closes on the first click outside it, unless that click
	// lands on the filter bar, which pins the bar until Escape or its close
	// button. Text or a filter holds the search open through any click.
	useEffect(() => {
		if (searchBar !== 'open') return;

		const handlePointerDown = (event: PointerEvent) => {
			const target = event.target as Node;
			if (searchFieldRef.current?.contains(target)) return;
			if (searchBarRef.current?.contains(target)) {
				pinSearch();
			} else if (!searchHeld) {
				exitSearchMode();
			}
		};
		// Reaching the bar from the keyboard engages it the same way.
		const handleFocusIn = (event: FocusEvent) => {
			if (searchBarRef.current?.contains(event.target as Node)) {
				pinSearch();
			}
		};

		document.addEventListener('pointerdown', handlePointerDown, true);
		document.addEventListener('focusin', handleFocusIn);
		return () => {
			document.removeEventListener(
				'pointerdown',
				handlePointerDown,
				true
			);
			document.removeEventListener('focusin', handleFocusIn);
		};
	}, [searchBar, searchHeld, pinSearch, exitSearchMode]);

	const handleSearchKeyDown = useCallback(
		(event: KeyboardEvent<HTMLInputElement>) => {
			if (event.key === 'Escape') {
				event.currentTarget.blur();
				exitSearchMode();
			}
		},
		[exitSearchMode]
	);

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

	// Allow rendering if we have a currentPath, a virtual view, or a special mode
	// (tag/recents/filtered). Only the plain "browse" mode without a path is empty.
	if (!currentPath && !isVirtualView && mode.type === 'browse') {
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
								<div ref={searchFieldRef}>
									<ExpandableSearchField
										ref={searchInputRef}
										expanded={searchBar !== 'closed'}
										onExpand={openSearch}
										placeholder={
											currentPath
												? 'Search in current folder...'
												: 'Search...'
										}
										value={searchValue}
										onChange={handleSearchChange}
										// Clearing keeps the field in use: the search stays
										// open for the next query or its filters.
										onClear={() =>
											searchInputRef.current?.focus()
										}
										onKeyDown={handleSearchKeyDown}
									/>
								</div>
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
				{searchBar !== 'closed' && <SearchToolbar ref={searchBarRef} />}
				<PlanPreviewBanner />
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
