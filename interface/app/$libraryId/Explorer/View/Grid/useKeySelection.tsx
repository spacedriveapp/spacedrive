import { useGrid } from '@virtual-grid/react';
import { useCallback, useEffect, useRef } from 'react';
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

interface UpdateActiveItemOptions {
	/**
	 * The index of the item to update. If not provided, the index will be reset.
	 * @default null
	 */
	itemIndex?: number | null;
	/**
	 * Whether to update the first active item.
	 * @default false
	 */
	updateFirstItem?: boolean;
	/**
	 * Whether to set the first item as changed. This is used to reset the selection.
	 * @default false
	 */
	setFirstItemAsChanged?: boolean;
}

export const useKeySelection = (grid: Grid, options: Options = { scrollToEnd: false }) => {
	const { explorerOperatingSystem } = useExplorerOperatingSystem();

	const explorer = useExplorerContext();
	const explorerView = useExplorerViewContext();

	// The item that further selection will move from (shift + arrow for example).
	const activeItem = useRef<ExplorerItem | null>(null);

	// The index of the active item. This is stored so we don't have to look
	// for the index every time we want to move to the next item.
	const activeItemIndex = useRef<number | null>(null);

	// The first active item that acts as a head.
	// Only used for windows OS to keep track of the first selected item.
	const firstActiveItem = useRef<ExplorerItem | null>(null);

	// The index of the first active item.
	// Only used for windows OS to keep track of the first selected item index.
	const firstActiveItemIndex = useRef<number | null>(null);

	// Whether the first active item has been changed.
	// Only used for windows OS to keep track whether selection should be reset.
	const hasFirstActiveItemChanged = useRef(true);

	// Reset active item when selection changes, as the active item
	// might not be in the selection anymore (further lookups are handled in handleNavigation).
	useEffect(() => {
		activeItem.current = null;
	}, [explorer.selectedItems]);

	// Reset active item index when items change,
	// as we can't guarantee the item is still in the same position
	useEffect(() => {
		activeItemIndex.current = null;
		firstActiveItemIndex.current = null;
	}, [explorer.items]);

	const updateFirstActiveItem = useCallback(
		(item: ExplorerItem | null, options: UpdateActiveItemOptions = {}) => {
			if (explorerOperatingSystem !== 'windows') return;

			firstActiveItem.current = item;
			firstActiveItemIndex.current = options.itemIndex ?? null;
			if (options.setFirstItemAsChanged) hasFirstActiveItemChanged.current = true;
		},
		[explorerOperatingSystem]
	);

	const updateActiveItem = useCallback(
		(item: ExplorerItem | null, options: UpdateActiveItemOptions = {}) => {
			// Timeout so the useEffect doesn't override it
			setTimeout(() => {
				activeItem.current = item;
				activeItemIndex.current = options.itemIndex ?? null;
			});

			if (options.updateFirstItem) updateFirstActiveItem(item, options);
		},
		[updateFirstActiveItem]
	);

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
		if (!explorerView.selectable || !explorer.items) return;

		e.preventDefault();
		e.stopPropagation();

		// Select first item in grid if no items are selected, on down/right keybind
		// TODO: Handle when no items are selected and up/left keybind is executed (should select last item in grid)
		if ((direction === 'down' || direction === 'right') && explorer.selectedItems.size === 0) {
			const item = grid.getItem(0);
			if (!item?.data) return;

			explorer.resetSelectedItems([item.data]);
			scrollToItem(item);

			updateActiveItem(item.data, { itemIndex: 0, updateFirstItem: true });

			return;
		}

		let currentItemIndex = activeItemIndex.current;

		// Check for any mismatches between the stored index and the current item
		if (currentItemIndex !== null) {
			if (activeItem.current) {
				const itemAtActiveIndex = explorer.items[currentItemIndex];
				const uniqueId = itemAtActiveIndex && explorer.getItemUniqueId(itemAtActiveIndex);
				if (uniqueId !== explorer.getItemUniqueId(activeItem.current)) {
					currentItemIndex = null;
				}
			} else {
				currentItemIndex = null;
			}
		}

		// Find index of current active item
		if (currentItemIndex === null) {
			let currentItem = activeItem.current;

			if (!currentItem) {
				const [item] = explorer.selectedItems;
				if (!item) return;

				currentItem = item;
			}

			const currentItemId = explorer.getItemUniqueId(currentItem);

			const index = explorer.items.findIndex((item) => {
				return explorer.getItemUniqueId(item) === currentItemId;
			});

			if (index === -1) return;

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

		const newSelectedItem = grid.getItem(newIndex);
		if (!newSelectedItem?.data) return;

		if (!e.shiftKey) {
			explorer.resetSelectedItems([newSelectedItem.data]);
		} else if (
			explorerOperatingSystem !== 'windows' &&
			!explorer.isItemSelected(newSelectedItem.data)
		) {
			explorer.addSelectedItem(newSelectedItem.data);
		} else if (explorerOperatingSystem === 'windows') {
			let firstItemId = firstActiveItem.current
				? explorer.getItemUniqueId(firstActiveItem.current)
				: undefined;

			let firstItemIndex = firstActiveItemIndex.current;

			// Check if the firstActiveItem is still in the selection. If not,
			// update the firstActiveItem to the current active item.
			if (firstActiveItem.current && explorer.selectedItems.has(firstActiveItem.current)) {
				let searchIndex = firstItemIndex === null;

				if (firstItemIndex !== null) {
					const itemAtIndex = explorer.items[firstItemIndex];
					const uniqueId = itemAtIndex && explorer.getItemUniqueId(itemAtIndex);
					if (uniqueId !== firstItemId) searchIndex = true;
				}

				// Search for the firstActiveItem index if we're missing the index or the ExplorerItem
				// at the stored index position doesn't match with the firstActiveItem
				if (searchIndex) {
					const item = explorer.items[currentItemIndex];
					if (!item) return;

					if (explorer.getItemUniqueId(item) === firstItemId) {
						firstItemIndex = currentItemIndex;
					} else {
						const index = explorer.items.findIndex((item) => {
							return explorer.getItemUniqueId(item) === firstItemId;
						});

						if (index === -1) return;

						firstItemIndex = index;
					}

					updateFirstActiveItem(firstActiveItem.current, { itemIndex: firstItemIndex });
				}
			} else {
				const item = explorer.items[currentItemIndex];
				if (!item) return;

				firstItemId = explorer.getItemUniqueId(item);
				firstItemIndex = currentItemIndex;

				updateFirstActiveItem(item, { itemIndex: firstItemIndex });
			}

			if (firstItemIndex === null) return;

			const addItems: ExplorerItem[] = [];
			const removeItems: ExplorerItem[] = [];

			// Determine if we moved further away from the first selected item.
			// This is used to determine if we should add or remove items from the selection.
			let movedAwayFromFirstItem = false;

			if (firstItemIndex === currentItemIndex) {
				movedAwayFromFirstItem = newIndex !== currentItemIndex;
			} else if (firstItemIndex < currentItemIndex) {
				movedAwayFromFirstItem = newIndex > currentItemIndex;
			} else {
				movedAwayFromFirstItem = newIndex < currentItemIndex;
			}

			// Determine if the new index is on the other side
			// of the firstActiveItem(head) based on the current index.
			const isIndexOverHead = (index: number) =>
				(currentItemIndex < firstItemIndex && index > firstItemIndex) ||
				(currentItemIndex > firstItemIndex && index < firstItemIndex);

			const itemsCount =
				Math.abs(currentItemIndex - newIndex) + (isIndexOverHead(newIndex) ? 1 : 0);

			for (let i = 0; i < itemsCount; i++) {
				const _i = i + (movedAwayFromFirstItem ? 1 : 0);
				const index = currentItemIndex + (currentItemIndex < newIndex ? _i : -_i);

				const item = explorer.items[index];
				if (!item || explorer.getItemUniqueId(item) === firstItemId) continue;

				const addItem = isIndexOverHead(index) || movedAwayFromFirstItem;
				(addItem ? addItems : removeItems).push(item);
			}

			if (hasFirstActiveItemChanged.current) {
				if (firstActiveItem.current) addItems.push(firstActiveItem.current);
				explorer.resetSelectedItems(addItems);
				hasFirstActiveItemChanged.current = false;
			} else {
				if (addItems.length > 0) explorer.addSelectedItem(addItems);
				if (removeItems.length > 0) explorer.removeSelectedItem(removeItems);
			}
		}

		updateActiveItem(newSelectedItem.data, { itemIndex: newIndex });
		updateFirstActiveItem(
			e.shiftKey ? firstActiveItem.current ?? newSelectedItem.data : newSelectedItem.data,
			{ itemIndex: e.shiftKey ? firstActiveItemIndex.current ?? currentItemIndex : newIndex }
		);

		scrollToItem(newSelectedItem);
	};

	// Debounce keybinds to prevent weird execution order
	const debounce = useDebouncedCallback((fn: () => void) => fn(), 10);

	useShortcut('explorerUp', (e) => debounce(() => handleNavigation(e, 'up')));
	useShortcut('explorerDown', (e) => debounce(() => handleNavigation(e, 'down')));
	useShortcut('explorerLeft', (e) => debounce(() => handleNavigation(e, 'left')));
	useShortcut('explorerRight', (e) => debounce(() => handleNavigation(e, 'right')));

	return { updateActiveItem, updateFirstActiveItem };
};
