import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useKeys } from 'rooks';
import {
	ExplorerLayout,
	explorerLayout,
	getItemObject,
	useSelector,
	type Object
} from '@sd/client';
import { dialogManager } from '@sd/ui';
import { Loader } from '~/components';
import { useKeyCopyCutPaste, useKeyMatcher, useShortcut } from '~/hooks';
import { useRoutingContext } from '~/RoutingContext';
import { isNonEmpty } from '~/util';

import CreateDialog from '../../settings/library/tags/CreateDialog';
import { useExplorerContext } from '../Context';
import { QuickPreview } from '../QuickPreview';
import { useQuickPreviewContext } from '../QuickPreview/Context';
import { getQuickPreviewStore, useQuickPreviewStore } from '../QuickPreview/store';
import { explorerStore } from '../store';
import { useExplorerDroppable } from '../useExplorerDroppable';
import { useExplorerSearchParams } from '../util';
import { ViewContext, type ExplorerViewContext } from './Context';
import { DragScrollable } from './DragScrollable';
import { GridView } from './GridView';
import { ListView } from './ListView';
import { MediaView } from './MediaView';
import { useViewItemDoubleClick } from './ViewItem';

export interface ExplorerViewProps
	extends Omit<ExplorerViewContext, 'selectable' | 'ref' | 'padding'> {
	emptyNotice?: JSX.Element;
}

export const View = ({ emptyNotice, ...contextProps }: ExplorerViewProps) => {
	const explorer = useExplorerContext();
	const [isContextMenuOpen, isRenaming, drag] = useSelector(explorerStore, (s) => [
		s.isContextMenuOpen,
		s.isRenaming,
		s.drag
	]);
	const { layoutMode } = explorer.useSettingsSnapshot();

	const quickPreview = useQuickPreviewContext();
	const quickPreviewStore = useQuickPreviewStore();

	const [{ path }] = useExplorerSearchParams();

	const { visible } = useRoutingContext();

	const ref = useRef<HTMLDivElement | null>(null);

	const [showLoading, setShowLoading] = useState(false);

	const selectable =
		explorer.selectable && !isContextMenuOpen && !isRenaming && !quickPreviewStore.open;

	// Can stay here until we add columns view
	// Once added, the provided parent related logic should move to useExplorerDroppable
	// that way we don't have to re-use the same logic for each view
	const { setDroppableRef } = useExplorerDroppable({
		...(explorer.parent?.type === 'Location' && {
			allow: ['Path', 'NonIndexedPath'],
			data: { type: 'location', path: path ?? '/', data: explorer.parent.location },
			disabled:
				drag?.type === 'dragging' &&
				explorer.parent.location.id === drag.sourceLocationId &&
				(path ?? '/') === drag.sourcePath
		}),
		...(explorer.parent?.type === 'Ephemeral' && {
			allow: ['Path', 'NonIndexedPath'],
			data: { type: 'location', path: explorer.parent.path },
			disabled: drag?.type === 'dragging' && explorer.parent.path === drag.sourcePath
		}),
		...(explorer.parent?.type === 'Tag' && {
			allow: 'Path',
			data: { type: 'tag', data: explorer.parent.tag },
			disabled: drag?.type === 'dragging' && explorer.parent.tag.id === drag.sourceTagId
		})
	});

	useShortcuts();

	useEffect(() => {
		if (!visible || !isContextMenuOpen || explorer.selectedItems.size !== 0) return;

		// Close context menu when no items are selected
		document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
		explorerStore.isContextMenuOpen = false;
	}, [explorer.selectedItems, isContextMenuOpen, visible]);

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
			explorerStore.isRenaming = false;
			explorerStore.isContextMenuOpen = false;
			explorerStore.isDragSelecting = false;
		};
	}, [layoutMode]);

	// Handle wheel scroll while dragging items
	useEffect(() => {
		const element = explorer.scrollRef.current;
		if (!element || drag?.type !== 'dragging') return;

		const handleWheel = (e: WheelEvent) => {
			element.scrollBy({ top: e.deltaY });
		};

		element.addEventListener('wheel', handleWheel);
		return () => element.removeEventListener('wheel', handleWheel);
	}, [explorer.scrollRef, drag?.type]);

	if (!explorer.layouts[layoutMode]) return null;

	return (
		<ViewContext.Provider value={{ ref, ...contextProps, selectable }}>
			<div
				ref={ref}
				className="flex flex-1"
				onMouseDown={(e) => {
					if (e.button === 2 || (e.button === 0 && e.shiftKey)) return;
					explorer.selectedItems.size !== 0 && explorer.resetSelectedItems();
				}}
			>
				<div ref={setDroppableRef} className="h-full w-full">
					{explorer.items === null || (explorer.items && explorer.items.length > 0) ? (
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

			{/* TODO: Move when adding columns view */}
			<DragScrollable />

			{quickPreview.ref && createPortal(<QuickPreview />, quickPreview.ref)}
		</ViewContext.Provider>
	);
};

const useShortcuts = () => {
	const explorer = useExplorerContext();
	const isRenaming = useSelector(explorerStore, (s) => s.isRenaming);
	const quickPreviewStore = useQuickPreviewStore();

	const meta = useKeyMatcher('Meta');
	const { doubleClick } = useViewItemDoubleClick();

	useKeyCopyCutPaste();

	useShortcut('toggleQuickPreview', (e) => {
		if (isRenaming || dialogManager.isAnyDialogOpen()) return;
		e.preventDefault();
		getQuickPreviewStore().open = !quickPreviewStore.open;
	});

	useShortcut('openObject', (e) => {
		if (isRenaming || quickPreviewStore.open) return;
		e.stopPropagation();
		e.preventDefault();
		doubleClick();
	});

	useShortcut('showImageSlider', (e) => {
		if (isRenaming) return;
		e.stopPropagation();
		explorerLayout.showImageSlider = !explorerLayout.showImageSlider;
	});

	useKeys([meta.key, 'KeyN'], () => {
		if (isRenaming || quickPreviewStore.open) return;

		const objects: Object[] = [];

		for (const item of explorer.selectedItems) {
			const object = getItemObject(item);
			if (!object) return;
			objects.push(object);
		}

		if (!isNonEmpty(objects)) return;

		dialogManager.create((dp) => (
			<CreateDialog {...dp} items={objects.map((item) => ({ type: 'Object', item }))} />
		));
	});
};
