import { useGrid } from '@virtual-grid/react';
import { useDebouncedCallback } from 'use-debounce';
import { ExplorerItem } from '@sd/client';
import { useShortcut } from '~/hooks';

import { useExplorerContext } from '../../Context';
import { useExplorerOperatingSystem } from '../../useExplorerOperatingSystem';
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

	const { explorerOperatingSystem } = useExplorerOperatingSystem();

	const scrollToItem = (index: number) => {
		if (!explorer.scrollRef.current || !explorerView.ref.current) return;

		const item = grid.getItem(index);
		if (!item) return;

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
		if (!explorerView.selectable || !explorer.items) return;

		e.preventDefault();
		e.stopPropagation();

		// Select first item in grid if no items are selected, on down/right keybind
		// TODO: Handle when no items are selected and up/left keybind is executed (should select last item in grid)
		if (explorer.selectedItems.size === 0) {
			if (direction !== 'down' && direction !== 'right') return;

			const item = explorer.items[0];
			if (!item) return;

			scrollToItem(0);

			explorer.resetSelectedItems([item]);

			explorerView.updateActiveItem(explorer.getItemUniqueId(item), {
				updateFirstItem: true
			});

			return;
		}

		const currentItemIndex = explorerView.getActiveItemIndex();
		if (currentItemIndex === undefined) return;

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

		// Adjust index if it's out of bounds
		if (direction === 'down' && newIndex > explorer.items.length - 1) {
			// Check if we're at the last row
			if (grid.getItem(currentItemIndex)?.row === grid.rowCount - 1) return;

			// By default select the last index in the grid if running on windows,
			// otherwise only if we're out of bounds by one item
			if (
				explorerOperatingSystem === 'windows' ||
				newIndex - (explorer.items.length - 1) === 1
			) {
				newIndex = explorer.items.length - 1;
			}
		}

		const newSelectedItem = explorer.items[newIndex];
		if (!newSelectedItem) return;

		scrollToItem(newIndex);

		if (!e.shiftKey) {
			explorer.resetSelectedItems([newSelectedItem]);
		} else if (
			explorerOperatingSystem !== 'windows' &&
			!explorer.isItemSelected(newSelectedItem)
		) {
			explorer.addSelectedItem(newSelectedItem);
		} else if (explorerOperatingSystem === 'windows') {
			explorerView.handleWindowsGridShiftSelection(newIndex);
			return;
		}

		explorerView.updateActiveItem(explorer.getItemUniqueId(newSelectedItem), {
			updateFirstItem: true
		});
	};

	// Debounce keybinds to prevent weird execution order
	const debounce = useDebouncedCallback((fn: () => void) => fn(), 10);

	useShortcut('explorerUp', (e) => debounce(() => handleNavigation(e, 'up')));
	useShortcut('explorerDown', (e) => debounce(() => handleNavigation(e, 'down')));
	useShortcut('explorerLeft', (e) => debounce(() => handleNavigation(e, 'left')));
	useShortcut('explorerRight', (e) => debounce(() => handleNavigation(e, 'right')));
};
