import type {Location} from '@sd/ts-client';
import clsx from 'clsx';
import {AnimatePresence, motion} from 'framer-motion';
import {useEffect, useMemo} from 'react';
import {Outlet, useLocation, useParams} from 'react-router-dom';
import {Inspector} from './components/Inspector/Inspector';
import {JobsProvider} from './components/JobManager/hooks/JobsContext';
import {
	PREVIEW_LAYER_ID,
	QuickPreviewController,
	QuickPreviewSyncer
} from './components/QuickPreview';
import {SpacesSidebar} from './components/SpacesSidebar';
import {
	TabBar,
	TabDefaultsSync,
	TabNavigationSync
} from './components/TabManager';
import {usePlatform} from './contexts/PlatformContext';
import {
	SHELL_INSPECTOR_WIDTH,
	SHELL_SIDEBAR_WIDTH,
	SHELL_TOOLBAR_HEIGHT,
	ShellGeometryContext
} from './contexts/ShellGeometryContext';
import {useNormalizedQuery} from './contexts/SpacedriveContext';
import {WebContextMenuProvider} from './contexts/WebContextMenuContext';
import {ExplorerProvider, useExplorer} from './routes/explorer';
import {KeyboardHandler} from './routes/explorer/KeyboardHandler';
import {SelectionProvider} from './routes/explorer/SelectionContext';
import {TagAssignmentMode} from './routes/explorer/TagAssignmentMode';
import {useShellLayout} from './stores/shellLayoutStore';
import {TopBar, TopBarProvider} from './TopBar';

function ShellLayoutContent() {
	const location = useLocation();
	const params = useParams();
	const platform = usePlatform();
	const layoutMode = useShellLayout();
	const isInset = layoutMode === 'inset';
	const {
		sidebarVisible,
		inspectorVisible,
		setInspectorVisible,
		quickPreviewFileId,
		tagModeActive,
		setTagModeActive,
		viewMode,
		currentPath
	} = useExplorer();

	// Check if we're on Overview (hide inspector) or in Knowledge view (has its own inspector)
	const isOverview = location.pathname === '/';
	const isKnowledgeView = viewMode === 'knowledge';

	// Fetch locations to get current location info
	const locationsQuery = useNormalizedQuery<null, {locations: Location[]}>({
		query: 'locations.list',
		input: null,
		resourceType: 'location'
	});

	// Get current location if we're on a location route or browsing within a location
	const currentLocation = useMemo(() => {
		const locations = locationsQuery.data?.locations || [];

		// First try to match by route param (for /location/:id routes)
		if (params.locationId) {
			const loc = locations.find((loc) => loc.id === params.locationId);
			if (loc) return loc;
		}

		// If no route match, try to find location by matching current path
		if (currentPath && 'Physical' in currentPath) {
			const pathStr = currentPath.Physical.path;
			// Find location with longest matching prefix
			return (
				locations
					.filter((loc) => {
						if (!loc.sd_path || !('Physical' in loc.sd_path))
							return false;
						const locPath = loc.sd_path.Physical.path;
						return pathStr.startsWith(locPath);
					})
					.sort((a, b) => {
						const aPath =
							'Physical' in a.sd_path!
								? a.sd_path!.Physical.path
								: '';
						const bPath =
							'Physical' in b.sd_path!
								? b.sd_path!.Physical.path
								: '';
						return bPath.length - aPath.length;
					})[0] || null
			);
		}

		return null;
	}, [params.locationId, locationsQuery.data, currentPath]);

	useEffect(() => {
		// Listen for inspector window close events
		if (!platform.onWindowEvent) return;

		let unlisten: (() => void) | undefined;

		(async () => {
			try {
				unlisten = await platform.onWindowEvent!(
					'inspector-window-closed',
					() => {
						// Show embedded inspector when floating window closes
						setInspectorVisible(true);
					}
				);
			} catch (err) {
				console.error('Failed to setup inspector close listener:', err);
			}
		})();

		return () => {
			unlisten?.();
		};
	}, [platform, setInspectorVisible]);

	const handlePopOutInspector = async () => {
		if (!platform.showWindow) return;

		try {
			await platform.showWindow({
				type: 'Inspector',
				item_id: null
			});
			// Hide the embedded inspector when popped out
			setInspectorVisible(false);
		} catch (err) {
			console.error('Failed to pop out inspector:', err);
		}
	};

	const isPreviewActive = !!quickPreviewFileId;
	const isSizeViewActive = viewMode === 'size';
	const showInspector = inspectorVisible && !isOverview && !isKnowledgeView;
	const geometry = useMemo(
		() => ({
			mode: layoutMode,
			overlayLeft: !isInset && sidebarVisible ? SHELL_SIDEBAR_WIDTH : 0,
			overlayRight: showInspector ? SHELL_INSPECTOR_WIDTH : 0
		}),
		[layoutMode, isInset, sidebarVisible, showInspector]
	);

	return (
		<ShellGeometryContext.Provider value={geometry}>
			<div
				data-shell-layout={layoutMode}
				className={clsx(
					'text-sidebar-ink relative flex h-screen select-none flex-col overflow-hidden border border-transparent',
					isInset ? 'bg-sidebar' : 'bg-app',
					platform.platform === 'tauri' && 'rounded-[10px]'
				)}
			>
				<div className="flex min-h-0 flex-1 overflow-hidden">
					<AnimatePresence initial={false} mode="popLayout">
						{sidebarVisible && (
							<motion.div
								initial={{x: -SHELL_SIDEBAR_WIDTH, width: 0}}
								animate={{x: 0, width: SHELL_SIDEBAR_WIDTH}}
								exit={{x: -SHELL_SIDEBAR_WIDTH, width: 0}}
								transition={{
									duration: 0.3,
									ease: [0.25, 1, 0.5, 1]
								}}
								className="relative z-[65] shrink-0 overflow-hidden"
							>
								<SpacesSidebar
									presentation={isInset ? 'flat' : 'floating'}
									isPreviewActive={
										isPreviewActive || isSizeViewActive
									}
								/>
							</motion.div>
						)}
					</AnimatePresence>

					{/* Changing the containing block keeps routes and portal children mounted. */}
					<div
						data-shell-workspace
						className={clsx(
							'flex min-h-0 min-w-0 flex-1',
							isInset &&
								'border-app-line bg-app relative my-[10px] mr-[10px] overflow-hidden rounded-2xl border',
							isInset && !sidebarVisible && 'ml-[10px]'
						)}
					>
						<div
							id={PREVIEW_LAYER_ID}
							className="pointer-events-none absolute inset-0 z-40 [&>*]:pointer-events-auto"
						/>
						<div
							id="size-view-layer"
							className="pointer-events-none absolute inset-0 z-[35] [&>*]:pointer-events-auto"
						/>
						<div className="from-app pointer-events-none absolute left-0 right-0 top-0 z-[37] h-32 bg-gradient-to-b to-transparent" />

						<TopBar
							sidebarWidth={
								geometry.overlayLeft +
								(!isInset && sidebarVisible ? 4 : 0)
							}
							inspectorWidth={
								geometry.overlayRight +
								(!isInset && showInspector ? 4 : 0)
							}
							reserveWindowControls={!sidebarVisible}
						/>

						{isSizeViewActive && (
							<div
								className="pointer-events-none absolute left-0 right-0 z-[45] [&>*]:pointer-events-auto"
								style={{
									top: SHELL_TOOLBAR_HEIGHT,
									paddingLeft: geometry.overlayLeft,
									paddingRight: geometry.overlayRight,
									transition: 'padding 0.3s ease-out'
								}}
							>
								<TabBar />
							</div>
						)}

						<div
							className={clsx(
								'relative flex min-w-0 flex-1 flex-col overflow-hidden pt-12',
								isSizeViewActive ? 'z-[30]' : 'z-[38]'
							)}
						>
							{/* Tab Bar - nested inside content area like Finder (hidden in size view) */}
							{!isSizeViewActive && <TabBar />}

							{/* Router content renders here */}
							<div className="relative flex-1 overflow-hidden">
								<Outlet />

								{/* Tag Assignment Mode - positioned at bottom of main content area */}
								<TagAssignmentMode
									isActive={tagModeActive}
									onExit={() => setTagModeActive(false)}
								/>
							</div>
						</div>

						<AnimatePresence initial={false}>
							{showInspector && (
								<motion.div
									initial={{width: 0}}
									animate={{width: SHELL_INSPECTOR_WIDTH}}
									exit={{width: 0}}
									transition={{
										duration: 0.3,
										ease: [0.25, 1, 0.5, 1]
									}}
									className="relative z-[65] shrink-0 overflow-hidden"
								>
									<div
										className={clsx(
											'flex h-full flex-col',
											!isInset && 'p-2'
										)}
										style={{width: SHELL_INSPECTOR_WIDTH}}
									>
										<Inspector
											presentation={
												isInset ? 'inset' : 'floating'
											}
											currentLocation={currentLocation}
											onPopOut={handlePopOutInspector}
											isPreviewActive={
												isPreviewActive ||
												isSizeViewActive
											}
										/>
									</div>
								</motion.div>
							)}
						</AnimatePresence>
					</div>
				</div>

				<KeyboardHandler />
				<QuickPreviewSyncer />
				<QuickPreviewController
					sidebarWidth={geometry.overlayLeft}
					inspectorWidth={geometry.overlayRight}
				/>
			</div>
		</ShellGeometryContext.Provider>
	);
}

export function ShellLayout() {
	return (
		<JobsProvider>
			<TopBarProvider>
				<SelectionProvider>
					<ExplorerProvider>
						<WebContextMenuProvider>
							{/* Sync tab navigation and defaults with router */}
							<TabNavigationSync />
							<TabDefaultsSync />
							<ShellLayoutContent />
						</WebContextMenuProvider>
					</ExplorerProvider>
				</SelectionProvider>
			</TopBarProvider>
		</JobsProvider>
	);
}
