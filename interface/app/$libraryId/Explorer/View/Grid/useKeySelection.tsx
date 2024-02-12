import { useGrid } from '@virtual-grid/react';
import { useEffect, useRef } from 'react';
import { ExplorerItem } from '@sd/client';
import { useShortcut } from '~/hooks';

import { useExplorerContext } from '../../Context';
import { useQuickPreviewStore } from '../../QuickPreview/store';
import { uniqueId } from '../../util';
import { useExplorerViewContext } from '../Context';

type Grid = ReturnType<typeof useGrid<string, ExplorerItem | undefined>>;

interface Options {
	/**
	 * Whether to scroll to the start/end of the grid on first/last row.
	 * @default false
	 */
	scrollToEnd?: boolean;
}

export const useKeySelection = (grid: Grid, options: Options = { scrollToEnd: false }) => {
	const explorer = useExplorerContext();
	const explorerView = useExplorerViewContext();
	const quickPreview = useQuickPreviewStore();

	// The item that further selection will move from (shift + arrow for example).
	const activeItem = useRef<ExplorerItem | null>(null);

	// The index of the active item. This is stored so we don't have to look
	// for the index every time we want to move to the next item.
	const activeItemIndex = useRef<number | null>(null);

	useEffect(() => {
		if (quickPreview.open) return;
		activeItem.current = [...explorer.selectedItems][0] ?? null;
	}, [explorer.selectedItems, quickPreview.open]);

	useEffect(() => {
		activeItemIndex.current = null;
	}, [explorer.items, explorer.selectedItems]);

	const scrollToItem = (item: NonNullable<ReturnType<Grid['getItem']>>) => {
		if (!explorer.scrollRef.current || !explorerView.ref.current) return;

		const { top: viewTop } = explorerView.ref.current.getBoundingClientRect();
		const { height: scrollHeight } = explorer.scrollRef.current.getBoundingClientRect();

		const itemTop = item.rect.top + viewTop;
		const itemBottom = item.rect.bottom + viewTop;

		const scrollTop = explorerView.scrollPadding?.top ?? 0;
		const scrollBottom = scrollHeight - (explorerView.scrollPadding?.bottom ?? 0);

		// Handle scroll when item is above viewport
		if (itemTop < scrollTop) {
			const offset = !item.row
				? (options.scrollToEnd && (grid.padding.top ?? 0)) || 0
				: (grid.gap.y ?? 0) / 2;

			explorer.scrollRef.current.scrollBy({ top: itemTop - scrollTop - offset });

			return;
		}

		// Handle scroll when item is bellow viewport
		if (itemBottom > scrollBottom) {
			const offset =
				item.row === grid.rowCount - 1
					? (options.scrollToEnd && (grid.padding.bottom ?? 0)) || 0
					: (grid.gap.y ?? 0) / 2;

			explorer.scrollRef.current.scrollBy({ top: itemBottom - scrollBottom + offset });
		}
	};

	const handleNavigation = (e: KeyboardEvent, direction: 'up' | 'down' | 'left' | 'right') => {
		if (!explorerView.selectable) return;

		e.preventDefault();
		e.stopPropagation();

		// Select first item in grid if no items are selected, on down/right keybind
		// TODO: Handle when no items are selected and up/left keybind is executed (should select last item in grid)
		if ((direction === 'down' || direction === 'right') && explorer.selectedItems.size === 0) {
			const item = grid.getItem(0);
			if (!item?.data) return;

			explorer.resetSelectedItems([item.data]);
			scrollToItem(item);

			return;
		}

		let currentItemIndex = activeItemIndex.current;

		// Find current index if we don't have the index stored
		if (currentItemIndex === null) {
			const currentItem = activeItem.current;
			if (!currentItem) return;

			const index = explorer.items?.findIndex(
				(item) => uniqueId(item) === uniqueId(currentItem)
			);

			if (index === undefined || index === -1) return;

			currentItemIndex = index;
		}

		if (currentItemIndex === null) return;

		let newIndex = currentItemIndex;

		switch (direction) {
			case 'up':
				newIndex -= grid.columnCount;
				break;
			case 'down':
				newIndex += grid.columnCount;
				break;
			case 'left':
				newIndex -= 1;
				break;
			case 'right':
				newIndex += 1;
		}

		const newSelectedItem = grid.getItem(newIndex);
		if (!newSelectedItem?.data) return;

		if (!e.shiftKey) {
			explorer.resetSelectedItems([newSelectedItem.data]);
		} else if (!explorer.isItemSelected(newSelectedItem.data)) {
			explorer.addSelectedItem(newSelectedItem.data);
		}

		// Timeout so useEffects don't override it
		setTimeout(() => {
			activeItem.current = newSelectedItem.data!;
			activeItemIndex.current = newIndex;
		});

		scrollToItem(newSelectedItem);
	};

	useShortcut('explorerUp', (e) => handleNavigation(e, 'up'));
	useShortcut('explorerDown', (e) => handleNavigation(e, 'down'));
	useShortcut('explorerLeft', (e) => handleNavigation(e, 'left'));
	useShortcut('explorerRight', (e) => handleNavigation(e, 'right'));

	return { activeItem };
};
