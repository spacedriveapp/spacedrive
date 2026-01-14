import type {Location} from '@sd/ts-client';
import clsx from 'clsx';
import {AnimatePresence, motion} from 'framer-motion';
import {useEffect, useMemo} from 'react';
import {Outlet, useLocation, useParams} from 'react-router-dom';
import {Inspector} from './components/Inspector/Inspector';
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
import {useNormalizedQuery} from './contexts/SpacedriveContext';
import {ExplorerProvider, useExplorer} from './routes/explorer';
import {KeyboardHandler} from './routes/explorer/KeyboardHandler';
import {SelectionProvider} from './routes/explorer/SelectionContext';
import {TagAssignmentMode} from './routes/explorer/TagAssignmentMode';
import {TopBar, TopBarProvider} from './TopBar';

function ShellLayoutContent() {
	const location = useLocation();
	const params = useParams();
	const platform = usePlatform();
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
		wireMethod: 'query:locations.list',
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
				unlisten = await platform.onWindowEvent(
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

	return (
		<div className="text-sidebar-ink bg-app relative flex h-screen select-none flex-col overflow-hidden rounded-[10px] border border-transparent">
			{/* Preview layer - portal target for fullscreen preview, sits between content and sidebar/inspector */}
			<div
				id={PREVIEW_LAYER_ID}
				className="pointer-events-none absolute inset-0 z-40 [&>*]:pointer-events-auto"
			/>

			{/* Size view layer - portal target for fullscreen size view, sits below preview */}
			<div
				id="size-view-layer"
				className="pointer-events-none absolute inset-0 z-[35] [&>*]:pointer-events-auto"
			/>

			{/* Top fade mask - spans full width beyond sidebar/inspector */}
			<div
				className="pointer-events-none absolute left-0 right-0 top-0 z-[37] h-32 bg-gradient-to-b from-app to-transparent"
			/>

			<TopBar
				sidebarWidth={sidebarVisible ? 224 : 0}
				inspectorWidth={
					inspectorVisible && !isOverview && !isKnowledgeView
						? 284
						: 0
				}
				isPreviewActive={isPreviewActive || isSizeViewActive}
			/>

			{/* Tab Bar floated above size view when active */}
			{isSizeViewActive && (
				<div
					className="pointer-events-none absolute left-0 right-0 z-[45] [&>*]:pointer-events-auto"
					style={{
						top: 48, // TopBar height
						paddingLeft: sidebarVisible ? 220 : 0,
						paddingRight:
							inspectorVisible && !isOverview && !isKnowledgeView
								? 280
								: 0,
						transition: 'padding 0.3s ease-out'
					}}
				>
					<TabBar />
				</div>
			)}

			{/* Main content area with sidebar and content */}
			<div className="flex flex-1 overflow-hidden">
				<AnimatePresence initial={false} mode="popLayout">
					{sidebarVisible && (
						<motion.div
							initial={{x: -220, width: 0}}
							animate={{x: 0, width: 220}}
							exit={{x: -220, width: 0}}
							transition={{
								duration: 0.3,
								ease: [0.25, 1, 0.5, 1]
							}}
							className="relative z-[65] overflow-hidden"
						>
							<SpacesSidebar
								isPreviewActive={
									isPreviewActive || isSizeViewActive
								}
							/>
						</motion.div>
					)}
				</AnimatePresence>

				{/* Content area with tabs - positioned between sidebar and inspector */}
				<div
					className={clsx(
						'relative flex flex-1 flex-col overflow-hidden pt-12',
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

				{/* Keyboard handler (invisible, doesn't cause parent rerenders) */}
				<KeyboardHandler />

				{/* Syncs selection to QuickPreview - isolated to prevent frame rerenders */}
				<QuickPreviewSyncer />

				<AnimatePresence initial={false}>
					{/* Hide inspector on Overview screen and Knowledge view (has its own) */}
					{inspectorVisible && !isOverview && !isKnowledgeView && (
						<motion.div
							initial={{width: 0}}
							animate={{width: 280}}
							exit={{width: 0}}
							transition={{
								duration: 0.3,
								ease: [0.25, 1, 0.5, 1]
							}}
							className="relative z-[65] overflow-hidden"
						>
							<div className="flex h-full w-[280px] min-w-[280px] flex-col bg-transparent p-2">
								<Inspector
									currentLocation={currentLocation}
									onPopOut={handlePopOutInspector}
									isPreviewActive={
										isPreviewActive || isSizeViewActive
									}
								/>
							</div>
						</motion.div>
					)}
				</AnimatePresence>
			</div>

			{/* Quick Preview - isolated component to prevent frame rerenders on selection change */}
			<QuickPreviewController
				sidebarWidth={sidebarVisible ? 220 : 0}
				inspectorWidth={
					inspectorVisible && !isOverview && !isKnowledgeView
						? 280
						: 0
				}
			/>
		</div>
	);
}

export function ShellLayout() {
	return (
		<TopBarProvider>
			<SelectionProvider>
				<ExplorerProvider>
					{/* Sync tab navigation and defaults with router */}
					<TabNavigationSync />
					<TabDefaultsSync />
					<ShellLayoutContent />
				</ExplorerProvider>
			</SelectionProvider>
		</TopBarProvider>
	);
}
