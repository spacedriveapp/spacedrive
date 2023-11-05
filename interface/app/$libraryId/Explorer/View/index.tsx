import { useDndMonitor } from '@dnd-kit/core';
import clsx from 'clsx';
import { memo, useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useKey, useKeys } from 'rooks';
import {
	ExplorerLayout,
	getIndexedItemFilePath,
	getItemObject,
	useLibraryMutation,
	type Object
} from '@sd/client';
import { dialogManager, ModifierKeys } from '@sd/ui';
import { Loader } from '~/components';
import { useKeyCopyCutPaste, useKeyMatcher, useOperatingSystem } from '~/hooks';
import { isNonEmpty } from '~/util';

import CreateDialog from '../../settings/library/tags/CreateDialog';
import { useExplorerContext } from '../Context';
import { QuickPreview } from '../QuickPreview';
import { useQuickPreviewContext } from '../QuickPreview/Context';
import { useQuickPreviewStore } from '../QuickPreview/store';
import { getExplorerStore, useExplorerStore } from '../store';
import { useExplorerSearchParams } from '../util';
import { ViewContext, type ExplorerViewContext } from '../ViewContext';
import { DragOverlay } from './DragOverlay';
import GridView from './GridView';
import ListView from './ListView';
import { MediaView } from './MediaView';
import { explorerDroppableSchema, useExplorerDroppable } from './useExplorerDroppable';
import { useExplorerViewPadding } from './util';
import { useViewItemDoubleClick } from './ViewItem';

export interface ExplorerViewPadding {
	x?: number;
	y?: number;
	top?: number;
	bottom?: number;
	left?: number;
	right?: number;
}

export interface ExplorerViewProps
	extends Omit<ExplorerViewContext, 'selectable' | 'ref' | 'padding'> {
	className?: string;
	style?: React.CSSProperties;
	emptyNotice?: JSX.Element;
	padding?: number | ExplorerViewPadding;
}

export default memo(
	({ className, style, emptyNotice, padding, ...contextProps }: ExplorerViewProps) => {
		const explorer = useExplorerContext();
		const explorerStore = useExplorerStore();
		const { layoutMode } = explorer.useSettingsSnapshot();

		const quickPreview = useQuickPreviewContext();
		const quickPreviewStore = useQuickPreviewStore();

		const [{ path }] = useExplorerSearchParams();

		const ref = useRef<HTMLDivElement | null>(null);

		const [showLoading, setShowLoading] = useState(false);

		const cutFiles = useLibraryMutation('files.cutFiles');

		const viewPadding = useExplorerViewPadding(padding);

		// Can stay here until we add columns view
		// Once added, the provided parent related logic should move to useExplorerDroppable
		// that way we don't have to re-use the same logic for each view
		const { setDroppableRef } = useExplorerDroppable({
			...(explorer.parent?.type === 'Location' && {
				allow: 'Path',
				data: { type: 'location', path: path ?? '/', data: explorer.parent.location },
				disabled:
					explorerStore.drag?.type === 'dragging' &&
					explorer.parent.location.id === explorerStore.drag.sourceLocationId &&
					(path ?? '/') === explorerStore.drag.sourceParentPath
			})
		});

		useDndMonitor({
			onDragStart: () => {
				if (explorer.parent?.type !== 'Location') return;
				getExplorerStore().drag = {
					type: 'dragging',
					items: [...explorer.selectedItems],
					sourceParentPath: path ?? '/',
					sourceLocationId: explorer.parent.location.id
				};
			},
			onDragEnd: ({ over }) => {
				const { drag } = getExplorerStore();
				getExplorerStore().drag = null;

				if (!over || !drag || drag.type === 'touched') return;

				const drop = explorerDroppableSchema.parse(over.data.current);

				const location =
					drop.type === 'location' ? drop.data.id : drop.data.item.location_id;

				const path =
					drop.type === 'explorer-item'
						? drop.data.item.materialized_path + drop.data.item.name + '/'
						: drop.path;

				if (
					drop.type === 'location'
						? location === drag.sourceLocationId && path === drag.sourceParentPath
						: path === drag.sourceParentPath
				) {
					return;
				}

				const pathIds = drag.items
					.map((item) => getIndexedItemFilePath(item)?.id)
					.filter((id): id is number => id !== undefined); // Where is ts-reset

				cutFiles.mutate({
					source_location_id: drag.sourceLocationId,
					sources_file_path_ids: pathIds,
					target_location_id: location,
					target_location_relative_directory_path: path
				});
			},
			onDragCancel: () => (getExplorerStore().drag = null)
		});

		useKeyDownHandlers({
			disabled: explorerStore.isRenaming || quickPreviewStore.open
		});

		useEffect(() => {
			if (!explorerStore.isContextMenuOpen || explorer.selectedItems.size !== 0) return;
			// Close context menu when no items are selected
			document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
			getExplorerStore().isContextMenuOpen = false;
		}, [explorer.selectedItems, explorerStore.isContextMenuOpen]);

		useEffect(() => {
			if (explorer.isFetchingNextPage) {
				const timer = setTimeout(() => setShowLoading(true), 100);
				return () => clearTimeout(timer);
			} else setShowLoading(false);
		}, [explorer.isFetchingNextPage]);

		useEffect(() => {
			if (explorer.layouts[layoutMode]) return;
			// If the current layout mode is not available, switch to the first available layout mode
			const layout = (Object.keys(explorer.layouts) as ExplorerLayout[]).find(
				(key) => explorer.layouts[key]
			);
			explorer.settingsStore.layoutMode = layout ?? 'grid';
		}, [layoutMode, explorer.layouts, explorer.settingsStore]);

		useEffect(() => {
			return () => {
				const store = getExplorerStore();
				store.isRenaming = false;
				store.isContextMenuOpen = false;
				store.isDragSelecting = false;
			};
		}, [layoutMode]);

		// Reset drag state - has to be separate from above useEffect
		// because not all locations use the same layout
		useEffect(() => {
			return () => {
				getExplorerStore().drag = null;
			};
		}, []);

		if (!explorer.layouts[layoutMode]) return null;

		return (
			<ViewContext.Provider
				value={{
					ref,
					padding: viewPadding,
					selectable:
						explorer.selectable &&
						!explorerStore.isContextMenuOpen &&
						!explorerStore.isRenaming &&
						(!quickPreviewStore.open || explorer.selectedItems.size === 1),
					...contextProps
				}}
			>
				<div
					ref={ref}
					style={style}
					className={clsx('flex flex-1', className)}
					onMouseDown={(e) => {
						if (e.button === 2 || (e.button === 0 && e.shiftKey)) return;
						explorer.resetSelectedItems();
					}}
				>
					<div ref={setDroppableRef} className="flex flex-1">
						{explorer.items === null ||
						(explorer.items && explorer.items.length > 0) ? (
							<>
								{layoutMode === 'grid' && <GridView />}
								{layoutMode === 'list' && <ListView />}
								{layoutMode === 'media' && <MediaView />}
								{showLoading && (
									<Loader className="fixed bottom-10 left-0 w-[calc(100%+180px)]" />
								)}
							</>
						) : (
							emptyNotice
						)}
					</div>
				</div>

				<DragOverlay />

				{quickPreview.ref && createPortal(<QuickPreview />, quickPreview.ref)}
			</ViewContext.Provider>
		);
	}
);

const useKeyDownHandlers = ({ disabled }: { disabled: boolean }) => {
	const os = useOperatingSystem();
	const { key: metaKey } = useKeyMatcher('Meta');

	const explorer = useExplorerContext();

	const { doubleClick } = useViewItemDoubleClick();

	const handleNewTag = useCallback(
		async (event: KeyboardEvent) => {
			const objects: Object[] = [];

			for (const item of explorer.selectedItems) {
				const object = getItemObject(item);
				if (!object) return;
				objects.push(object);
			}

			if (
				!isNonEmpty(objects) ||
				event.key.toUpperCase() !== 'N' ||
				!event.getModifierState(os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control)
			)
				return;

			dialogManager.create((dp) => (
				<CreateDialog {...dp} items={objects.map((item) => ({ type: 'Object', item }))} />
			));
		},
		[os, explorer.selectedItems]
	);

	const handleExplorerShortcut = useCallback(
		(event: KeyboardEvent) => {
			if (
				event.key.toUpperCase() !== 'I' ||
				!event.getModifierState(os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control)
			)
				return;

			getExplorerStore().showInspector = !getExplorerStore().showInspector;
		},
		[os]
	);

	useKeyCopyCutPaste();

	useKey(['Enter'], (e) => {
		e.stopPropagation();
		if (os === 'windows' && !disabled) doubleClick();
	});

	useKeys([metaKey, 'KeyO'], (e) => {
		e.stopPropagation();
		if (os !== 'windows') doubleClick();
	});

	useEffect(() => {
		const handlers = [handleNewTag, handleExplorerShortcut];
		const handler = (event: KeyboardEvent) => {
			if (event.repeat || disabled) return;
			for (const handler of handlers) handler(event);
		};
		document.body.addEventListener('keydown', handler);
		return () => document.body.removeEventListener('keydown', handler);
	}, [disabled, handleNewTag, handleExplorerShortcut]);
};
