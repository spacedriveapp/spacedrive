import { SpacedriveProvider, type SpacedriveClient } from "./context";
import { ServerProvider } from "./ServerContext";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import {
	RouterProvider,
	Outlet,
	useLocation,
	useParams,
} from "react-router-dom";
import { useEffect, useMemo, memo, useRef } from "react";
import { Dialogs } from "@sd/ui";
import { Inspector, type InspectorVariant } from "./Inspector";
import { TopBarProvider, TopBar } from "./TopBar";
import { motion, AnimatePresence } from "framer-motion";
import { ExplorerProvider, useExplorer, Sidebar } from "./components/Explorer";
import {
	SelectionProvider,
	useSelection,
} from "./components/Explorer/SelectionContext";
import { KeyboardHandler } from "./components/Explorer/KeyboardHandler";
import { TagAssignmentMode } from "./components/Explorer/TagAssignmentMode";
import { SpacesSidebar } from "./components/SpacesSidebar";
import {
	QuickPreviewFullscreen,
	PREVIEW_LAYER_ID,
} from "./components/QuickPreview";
import { createExplorerRouter, explorerRoutes } from "./router";
import {
	useNormalizedQuery,
	useLibraryMutation,
	useSpacedriveClient,
} from "./context";
import { useSidebarStore } from "@sd/ts-client";
import { useSpaces } from "./components/SpacesSidebar/hooks/useSpaces";
import { useQueryClient } from "@tanstack/react-query";
import { usePlatform } from "./platform";
import type { LocationInfo, SdPath } from "@sd/ts-client";
import {
	DndContext,
	DragOverlay,
	PointerSensor,
	useSensor,
	useSensors,
	pointerWithin,
	rectIntersection,
} from "@dnd-kit/core";
import type { CollisionDetection } from "@dnd-kit/core";
import { useState } from "react";
import type { File } from "@sd/ts-client";
import { File as FileComponent } from "./components/Explorer/File";
import { DaemonDisconnectedOverlay } from "./components/DaemonDisconnectedOverlay";
import { DaemonStartupOverlay } from "./components/DaemonStartupOverlay";
import { useDaemonStatus } from "./hooks/useDaemonStatus";
import { useFileOperationDialog } from "./components/FileOperationModal";
import { House, Clock, Heart, Folders } from "@phosphor-icons/react";
import {
	TabManagerProvider,
	TabBar,
	TabNavigationSync,
	TabDefaultsSync,
	TabKeyboardHandler,
	useTabManager,
} from "./components/TabManager";

/**
 * QuickPreviewSyncer - Syncs selection changes to QuickPreview
 *
 * This component is isolated so selection changes only re-render this tiny component,
 * not the entire ExplorerLayout. When selection changes while QuickPreview is open,
 * we update the preview to show the newly selected file.
 */
function QuickPreviewSyncer() {
	const { quickPreviewFileId, openQuickPreview } = useExplorer();
	const { selectedFiles } = useSelection();

	useEffect(() => {
		if (!quickPreviewFileId) return;

		// When selection changes and QuickPreview is open, update preview to match selection
		if (
			selectedFiles.length === 1 &&
			selectedFiles[0].id !== quickPreviewFileId
		) {
			openQuickPreview(selectedFiles[0].id);
		}
	}, [selectedFiles, quickPreviewFileId, openQuickPreview]);

	return null;
}

/**
 * QuickPreviewController - Handles QuickPreview with navigation
 *
 * Isolated component that reads selection state for prev/next navigation.
 * Only re-renders when quickPreviewFileId changes, not on every selection change.
 */
const QuickPreviewController = memo(function QuickPreviewController({
	sidebarWidth,
	inspectorWidth,
}: {
	sidebarWidth: number;
	inspectorWidth: number;
}) {
	const { quickPreviewFileId, closeQuickPreview, currentFiles } =
		useExplorer();
	const { selectFile } = useSelection();

	// Early return if no preview - this component won't re-render on selection changes
	// because it's memoized and doesn't read selectedFiles directly
	if (!quickPreviewFileId) return null;

	const currentIndex = currentFiles.findIndex(
		(f) => f.id === quickPreviewFileId,
	);
	const hasPrevious = currentIndex > 0;
	const hasNext = currentIndex < currentFiles.length - 1;

	const handleNext = () => {
		if (hasNext && currentFiles[currentIndex + 1]) {
			selectFile(
				currentFiles[currentIndex + 1],
				currentFiles,
				false,
				false,
			);
		}
	};

	const handlePrevious = () => {
		if (hasPrevious && currentFiles[currentIndex - 1]) {
			selectFile(
				currentFiles[currentIndex - 1],
				currentFiles,
				false,
				false,
			);
		}
	};

	return (
		<QuickPreviewFullscreen
			fileId={quickPreviewFileId}
			isOpen={!!quickPreviewFileId}
			onClose={closeQuickPreview}
			onNext={handleNext}
			onPrevious={handlePrevious}
			hasPrevious={hasPrevious}
			hasNext={hasNext}
			sidebarWidth={sidebarWidth}
			inspectorWidth={inspectorWidth}
		/>
	);
});

interface AppProps {
	client: SpacedriveClient;
}

function ExplorerLayoutContent() {
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
		currentPath,
	} = useExplorer();

	// Check if we're on Overview (hide inspector) or in Knowledge view (has its own inspector)
	const isOverview = location.pathname === "/";
	const isKnowledgeView = viewMode === "knowledge";

	// Fetch locations to get current location info
	const locationsQuery = useNormalizedQuery<
		null,
		{ locations: LocationInfo[] }
	>({
		wireMethod: "query:locations.list",
		input: null,
		resourceType: "location",
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
		if (currentPath && "Physical" in currentPath) {
			const pathStr = currentPath.Physical.path;
			// Find location with longest matching prefix
			return (
				locations
					.filter((loc) => {
						if (!loc.sd_path || !("Physical" in loc.sd_path))
							return false;
						const locPath = loc.sd_path.Physical.path;
						return pathStr.startsWith(locPath);
					})
					.sort((a, b) => {
						const aPath =
							"Physical" in a.sd_path!
								? a.sd_path!.Physical.path
								: "";
						const bPath =
							"Physical" in b.sd_path!
								? b.sd_path!.Physical.path
								: "";
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
					"inspector-window-closed",
					() => {
						// Show embedded inspector when floating window closes
						setInspectorVisible(true);
					},
				);
			} catch (err) {
				console.error("Failed to setup inspector close listener:", err);
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
				type: "Inspector",
				item_id: null,
			});
			// Hide the embedded inspector when popped out
			setInspectorVisible(false);
		} catch (err) {
			console.error("Failed to pop out inspector:", err);
		}
	};

	const isPreviewActive = !!quickPreviewFileId;

	return (
		<div className="relative flex flex-col h-screen select-none overflow-hidden text-sidebar-ink bg-app rounded-[10px] border border-transparent frame">
			{/* Preview layer - portal target for fullscreen preview, sits between content and sidebar/inspector */}
			<div
				id={PREVIEW_LAYER_ID}
				className="absolute inset-0 z-40 pointer-events-none [&>*]:pointer-events-auto"
			/>

			<TopBar
				sidebarWidth={sidebarVisible ? 224 : 0}
				inspectorWidth={
					inspectorVisible && !isOverview && !isKnowledgeView
						? 284
						: 0
				}
				isPreviewActive={isPreviewActive}
			/>

			{/* Main content area with sidebar and content */}
			<div className="flex flex-1 overflow-hidden">
				<AnimatePresence initial={false} mode="popLayout">
					{sidebarVisible && (
						<motion.div
							initial={{ x: -220, width: 0 }}
							animate={{ x: 0, width: 220 }}
							exit={{ x: -220, width: 0 }}
							transition={{
								duration: 0.3,
								ease: [0.25, 1, 0.5, 1],
							}}
							className="relative z-50 overflow-hidden"
						>
							<SpacesSidebar isPreviewActive={isPreviewActive} />
						</motion.div>
					)}
				</AnimatePresence>

				{/* Content area with tabs - positioned between sidebar and inspector */}
				<div className="relative flex-1 flex flex-col overflow-hidden z-30 pt-12">
					{/* Tab Bar - nested inside content area like Finder */}
					<TabBar />

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
							initial={{ width: 0 }}
							animate={{ width: 280 }}
							exit={{ width: 0 }}
							transition={{
								duration: 0.3,
								ease: [0.25, 1, 0.5, 1],
							}}
							className="relative z-50 overflow-hidden"
						>
							<div className="w-[280px] min-w-[280px] flex flex-col h-full p-2 bg-transparent">
								<Inspector
									currentLocation={currentLocation}
									onPopOut={handlePopOutInspector}
									isPreviewActive={isPreviewActive}
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

/**
 * DndWrapper - Global drag-and-drop coordinator
 *
 * Handles all drag-and-drop operations in the Explorer using @dnd-kit/core.
 *
 * Drop Actions:
 *
 * 1. insert-before / insert-after
 *    - Pins a file to the sidebar before/after an existing item
 *    - Shows a blue line indicator
 *    - Data: { action, itemId }
 *
 * 2. move-into
 *    - Moves a file into a location/volume/folder
 *    - Shows a blue ring around the target
 *    - Data: { action, targetType, targetId, targetPath? }
 *    - targetType: "location" | "volume" | "folder"
 *    - targetPath: SdPath (for locations, directly usable)
 *
 * 3. type: "space" | "group"
 *    - Legacy: Drops on the space root or group area (no specific item)
 *    - Adds item to space/group
 *    - Data: { type, spaceId, groupId? }
 */
function DndWrapper({ children }: { children: React.ReactNode }) {
	const sensors = useSensors(
		useSensor(PointerSensor, {
			activationConstraint: {
				distance: 8, // Require 8px movement before activating drag
			},
		}),
	);
	const addItem = useLibraryMutation("spaces.add_item");
	const reorderItems = useLibraryMutation("spaces.reorder_items");
	const reorderGroups = useLibraryMutation("spaces.reorder_groups");
	const openFileOperation = useFileOperationDialog();
	const [activeItem, setActiveItem] = useState<any>(null);
	const client = useSpacedriveClient();
	const queryClient = useQueryClient();
	const { currentSpaceId } = useSidebarStore();
	const { data: spacesData } = useSpaces();
	const spaces = spacesData?.spaces;

	// Custom collision detection: prefer -top zones over -bottom zones to avoid double lines
	const customCollision: CollisionDetection = (args) => {
		const collisions = pointerWithin(args);
		if (!collisions || collisions.length === 0) return collisions;

		// If we have multiple collisions, prefer -top over -bottom
		const hasTop = collisions.find((c) => String(c.id).endsWith("-top"));
		const hasMiddle = collisions.find((c) =>
			String(c.id).endsWith("-middle"),
		);

		if (hasMiddle) return [hasMiddle]; // Middle zone takes priority
		if (hasTop) return [hasTop]; // Top zone over bottom
		return [collisions[0]]; // First collision
	};

	const handleDragStart = (event: any) => {
		setActiveItem(event.active.data.current);
	};

	const handleDragEnd = async (event: any) => {
		const { active, over } = event;

		setActiveItem(null);

		if (!over) return;

		// Handle sortable reordering (no drag data, just active/over IDs)
		if (active.id !== over.id && !active.data.current?.type) {
			console.log("[DnD] Sortable reorder:", {
				activeId: active.id,
				overId: over.id,
			});

			const libraryId = client.getCurrentLibraryId();
			const currentSpace =
				spaces?.find((s: any) => s.id === currentSpaceId) ??
				spaces?.[0];

			if (!currentSpace || !libraryId) return;

			const queryKey = [
				"query:spaces.get_layout",
				libraryId,
				{ space_id: currentSpace.id },
			];
			const layout = queryClient.getQueryData(queryKey) as any;

			if (!layout) return;

			// Check if we're reordering groups
			const groups = layout.groups?.map((g: any) => g.group) || [];
			const isGroupReorder = groups.some((g: any) => g.id === active.id);

			if (isGroupReorder) {
				console.log("[DnD] Reordering groups");

				const oldIndex = groups.findIndex(
					(g: any) => g.id === active.id,
				);
				const newIndex = groups.findIndex((g: any) => g.id === over.id);

				if (
					oldIndex !== -1 &&
					newIndex !== -1 &&
					oldIndex !== newIndex
				) {
					// Optimistically update the UI
					const newGroups = [...layout.groups];
					const [movedGroup] = newGroups.splice(oldIndex, 1);
					newGroups.splice(newIndex, 0, movedGroup);

					queryClient.setQueryData(queryKey, {
						...layout,
						groups: newGroups,
					});

					// Send reorder mutation
					try {
						await reorderGroups.mutateAsync({
							space_id: currentSpace.id,
							group_ids: newGroups.map((g: any) => g.group.id),
						});
						console.log("[DnD] Group reorder successful");
					} catch (err) {
						console.error("[DnD] Group reorder failed:", err);
						// Revert on error
						queryClient.setQueryData(queryKey, layout);
					}
				}

				return;
			}

			// Reordering space items
			if (layout?.space_items) {
				const items = layout.space_items;
				const oldIndex = items.findIndex(
					(item: any) => item.id === active.id,
				);

				// Extract item ID from over.id (could be a drop zone ID like "space-item-{id}-top")
				let overItemId = String(over.id);
				if (overItemId.startsWith("space-item-")) {
					// Extract the UUID from "space-item-{uuid}-top/bottom/middle"
					const parts = overItemId.split("-");
					// Remove "space" and "item" and the last part (top/bottom/middle)
					overItemId = parts.slice(2, -1).join("-");
				}

				const newIndex = items.findIndex(
					(item: any) => item.id === overItemId,
				);

				console.log("[DnD] Reorder space items:", {
					oldIndex,
					newIndex,
					activeId: active.id,
					extractedOverId: overItemId,
				});

				if (
					oldIndex !== -1 &&
					newIndex !== -1 &&
					oldIndex !== newIndex
				) {
					// Optimistically update the UI
					const newItems = [...items];
					const [movedItem] = newItems.splice(oldIndex, 1);
					newItems.splice(newIndex, 0, movedItem);

					queryClient.setQueryData(queryKey, {
						...layout,
						space_items: newItems,
					});

					// Send reorder mutation
					try {
						await reorderItems.mutateAsync({
							group_id: null, // Space-level items
							item_ids: newItems.map((item: any) => item.id),
						});
						console.log("[DnD] Space items reorder successful");
					} catch (err) {
						console.error("[DnD] Space items reorder failed:", err);
						// Revert on error
						queryClient.setQueryData(queryKey, layout);
					}
				}
			}

			return;
		}

		if (!active.data.current) return;

		const dragData = active.data.current;
		const dropData = over.data.current;

		console.log("[DnD] Drag end:", {
			dragType: dragData?.type,
			dropAction: dropData?.action,
			dropType: dropData?.type,
			spaceId: dropData?.spaceId,
			groupId: dropData?.groupId,
		});

		// Handle palette item drops (from customization panel)
		if (dragData?.type === "palette-item") {
			const libraryId = client.getCurrentLibraryId();
			const currentSpace =
				spaces?.find((s: any) => s.id === currentSpaceId) ??
				spaces?.[0];

			if (!currentSpace || !libraryId) return;

			console.log("[DnD] Adding palette item:", {
				itemType: dragData.itemType,
				spaceId: currentSpace.id,
				dropAction: dropData?.action,
				groupId: dropData?.groupId,
			});

			try {
				await addItem.mutateAsync({
					space_id: currentSpace.id,
					group_id: dropData?.groupId || null,
					item_type: dragData.itemType,
				});
				console.log("[DnD] Successfully added palette item");
			} catch (err) {
				console.error("[DnD] Failed to add palette item:", err);
			}
			return;
		}

		if (!dragData || dragData.type !== "explorer-file") return;

		// Add to space (root-level drop zones between groups)
		if (dropData?.action === "add-to-space") {
			if (!dropData.spaceId) return;

			console.log("[DnD] Adding to space root:", {
				spaceId: dropData.spaceId,
				sdPath: dragData.sdPath,
			});

			try {
				await addItem.mutateAsync({
					space_id: dropData.spaceId,
					group_id: null,
					item_type: { Path: { sd_path: dragData.sdPath } },
				});
				console.log("[DnD] Successfully added to space root");
			} catch (err) {
				console.error("[DnD] Failed to add to space:", err);
			}
			return;
		}

		// Add to group (empty group drop zone)
		if (dropData?.action === "add-to-group") {
			if (!dropData.spaceId || !dropData.groupId) return;

			console.log("[DnD] Adding to group:", {
				spaceId: dropData.spaceId,
				groupId: dropData.groupId,
				sdPath: dragData.sdPath,
			});

			try {
				await addItem.mutateAsync({
					space_id: dropData.spaceId,
					group_id: dropData.groupId,
					item_type: { Path: { sd_path: dragData.sdPath } },
				});
				console.log("[DnD] Successfully added to group");
			} catch (err) {
				console.error("[DnD] Failed to add to group:", err);
			}
			return;
		}

		// Insert before/after sidebar items (adds item to space/group)
		if (
			dropData?.action === "insert-before" ||
			dropData?.action === "insert-after"
		) {
			if (!dropData.spaceId) return;

			console.log("[DnD] Inserting item:", {
				action: dropData.action,
				spaceId: dropData.spaceId,
				groupId: dropData.groupId,
				sdPath: dragData.sdPath,
			});

			try {
				await addItem.mutateAsync({
					space_id: dropData.spaceId,
					group_id: dropData.groupId || null,
					item_type: { Path: { sd_path: dragData.sdPath } },
				});
				console.log("[DnD] Successfully inserted item");
				// TODO: Implement proper ordering relative to itemId
			} catch (err) {
				console.error("[DnD] Failed to add item:", err);
			}
			return;
		}

		// Move file into location/volume/folder
		if (dropData?.action === "move-into") {
			console.log("[DnD] Move-into action:", {
				targetType: dropData.targetType,
				targetId: dropData.targetId,
				targetPath: dropData.targetPath,
				hasTargetPath: !!dropData.targetPath,
				draggedFile: dragData.name,
			});

			const sources: SdPath[] = dragData.selectedFiles
				? dragData.selectedFiles.map((f: File) => f.sd_path)
				: [dragData.sdPath];

			const destination: SdPath = dropData.targetPath;

			if (!destination) {
				console.error("[DnD] No target path for move-into action");
				return;
			}

			// Determine operation based on modifier keys
			// For now default to copy (user can choose in modal)
			const operation = "copy";

			openFileOperation({
				operation,
				sources,
				destination,
			});
			return;
		}

		// Drop on space root area (adds to space)
		if (dropData?.type === "space" && dragData.type === "explorer-file") {
			console.log("[DnD] Adding to space (type=space):", {
				spaceId: dropData.spaceId,
				sdPath: dragData.sdPath,
			});

			try {
				await addItem.mutateAsync({
					space_id: dropData.spaceId,
					group_id: null,
					item_type: { Path: { sd_path: dragData.sdPath } },
				});
				console.log("[DnD] Successfully added to space");
			} catch (err) {
				console.error("[DnD] Failed to add item:", err);
			}
		}

		// Drop on group area (adds to group)
		if (dropData?.type === "group" && dragData.type === "explorer-file") {
			console.log("[DnD] Adding to group (type=group):", {
				spaceId: dropData.spaceId,
				groupId: dropData.groupId,
				sdPath: dragData.sdPath,
			});

			try {
				await addItem.mutateAsync({
					space_id: dropData.spaceId,
					group_id: dropData.groupId,
					item_type: { Path: { sd_path: dragData.sdPath } },
				});
				console.log("[DnD] Successfully added to group");
			} catch (err) {
				console.error("[DnD] Failed to add item to group:", err);
			}
		}
	};

	return (
		<DndContext
			sensors={sensors}
			collisionDetection={customCollision}
			onDragStart={handleDragStart}
			onDragEnd={handleDragEnd}
		>
			{children}
			<DragOverlay dropAnimation={null}>
				{activeItem?.type === "palette-item" ? (
					// Palette item preview
					<div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-accent text-white shadow-lg min-w-[180px]">
						{activeItem.itemType === "Overview" && (
							<House size={20} weight="bold" />
						)}
						{activeItem.itemType === "Recents" && (
							<Clock size={20} weight="bold" />
						)}
						{activeItem.itemType === "Favorites" && (
							<Heart size={20} weight="bold" />
						)}
						{activeItem.itemType === "FileKinds" && (
							<Folders size={20} weight="bold" />
						)}
						<span className="text-sm font-medium">
							{activeItem.itemType === "Overview" && "Overview"}
							{activeItem.itemType === "Recents" && "Recents"}
							{activeItem.itemType === "Favorites" && "Favorites"}
							{activeItem.itemType === "FileKinds" &&
								"File Kinds"}
						</span>
					</div>
				) : activeItem?.label ? (
					// Group or SpaceItem preview (from sortable context)
					<div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-sidebar/95 backdrop-blur-sm text-sidebar-ink shadow-lg border border-sidebar-line min-w-[180px]">
						<span className="text-sm font-medium">
							{activeItem.label}
						</span>
					</div>
				) : activeItem?.file ? (
					activeItem.gridSize ? (
						// Grid view preview
						<div style={{ width: activeItem.gridSize }}>
							<div className="flex flex-col items-center gap-2 p-1 rounded-lg relative">
								<div className="rounded-lg p-2">
									<FileComponent.Thumb
										file={activeItem.file}
										size={Math.max(
											activeItem.gridSize * 0.6,
											60,
										)}
									/>
								</div>
								<div className="text-sm truncate px-2 py-0.5 rounded-md bg-accent text-white max-w-full">
									{activeItem.name}
								</div>
								{/* Show count badge if dragging multiple files */}
								{activeItem.selectedFiles &&
									activeItem.selectedFiles.length > 1 && (
										<div className="absolute -top-2 -right-2 size-6 rounded-full bg-accent text-white text-xs font-bold flex items-center justify-center shadow-lg border-2 border-app">
											{activeItem.selectedFiles.length}
										</div>
									)}
							</div>
						</div>
					) : (
						// Column/List view preview
						<div className="flex items-center gap-2 px-3 py-1.5 rounded-md bg-accent text-white shadow-lg min-w-[200px] max-w-[300px]">
							<FileComponent.Thumb
								file={activeItem.file}
								size={24}
							/>
							<span className="text-sm font-medium truncate">
								{activeItem.name}
							</span>
							{/* Show count badge if dragging multiple files */}
							{activeItem.selectedFiles &&
								activeItem.selectedFiles.length > 1 && (
									<div className="ml-auto size-5 rounded-full bg-white text-accent text-xs font-bold flex items-center justify-center">
										{activeItem.selectedFiles.length}
									</div>
								)}
						</div>
					)
				) : null}
			</DragOverlay>
		</DndContext>
	);
}

export function ExplorerLayout() {
	return (
		<TopBarProvider>
			<SelectionProvider>
				<ExplorerProvider>
					{/* Sync tab navigation and defaults with router */}
					<TabNavigationSync />
					<TabDefaultsSync />
					<ExplorerLayoutContent />
				</ExplorerProvider>
			</SelectionProvider>
		</TopBarProvider>
	);
}

function ExplorerWithTabs() {
	const { router } = useTabManager();

	return (
		<DndWrapper>
			<RouterProvider router={router} />
		</DndWrapper>
	);
}

export function Explorer({ client }: AppProps) {
	const platform = usePlatform();
	const isTauri = platform.platform === "tauri";

	return (
		<SpacedriveProvider client={client}>
			<ServerProvider>
				{isTauri ? (
					// Tauri: Wait for daemon connection before rendering content
					<ExplorerWithDaemonCheck />
				) : (
					// Web: Render immediately (daemon connection handled differently)
					<>
						<TabManagerProvider routes={explorerRoutes}>
							<TabKeyboardHandler />
							<ExplorerWithTabs />
						</TabManagerProvider>
						<Dialogs />
						<ReactQueryDevtools
							initialIsOpen={false}
							buttonPosition="bottom-right"
						/>
					</>
				)}
			</ServerProvider>
		</SpacedriveProvider>
	);
}

/**
 * Tauri-specific wrapper that prevents Explorer from rendering until daemon is connected.
 * This avoids the connection storm where hundreds of queries try to execute before daemon is ready.
 */
function ExplorerWithDaemonCheck() {
	const daemonStatus = useDaemonStatus();
	const { isConnected, isStarting } = daemonStatus;

	return (
		<>
			{isConnected ? (
				// Daemon connected - render full app
				<>
					<TabManagerProvider routes={explorerRoutes}>
						<TabKeyboardHandler />
						<ExplorerWithTabs />
					</TabManagerProvider>
					<Dialogs />
					<ReactQueryDevtools
						initialIsOpen={false}
						buttonPosition="bottom-right"
					/>
				</>
			) : (
				// Daemon not connected - show appropriate overlay
				<>
					<DaemonStartupOverlay show={isStarting} />
					{!isStarting && (
						<DaemonDisconnectedOverlay
							daemonStatus={daemonStatus}
						/>
					)}
				</>
			)}
		</>
	);
}
