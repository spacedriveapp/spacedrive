import { MutableRefObject, useCallback, useRef } from 'react';

import { useExplorerContext } from '../Context';
import { useExplorerOperatingSystem } from '../useExplorerOperatingSystem';

type ActiveItem = string | null;

type UpdateActiveItem = ActiveItem | ((current: ActiveItem) => ActiveItem);

interface UpdateActiveItemOptions {
	/**
	 * Whether to update the first active item.
	 * @default false
	 */
	updateFirstItem?: boolean;
}

export function useActiveItem() {
	const explorer = useExplorerContext();

	const { explorerOperatingSystem } = useExplorerOperatingSystem();

	// The item that further selection will move from (shift + arrow for example).
	const activeItem = useRef<ActiveItem>(null);

	// The first active item that acts as a head.
	// Only used for windows OS to keep track of the first selected item.
	const firstActiveItem = useRef<ActiveItem>(null);

	const updateItem = useCallback((item: MutableRefObject<ActiveItem>, data: UpdateActiveItem) => {
		item.current = typeof data === 'function' ? data(firstActiveItem.current) : data;
	}, []);

	const updateFirstActiveItem = useCallback(
		(item: UpdateActiveItem) => {
			if (explorerOperatingSystem !== 'windows') return;
			updateItem(firstActiveItem, item);
		},
		[explorerOperatingSystem, updateItem]
	);

	const updateActiveItem = useCallback(
		(item: UpdateActiveItem, options: UpdateActiveItemOptions = {}) => {
			updateItem(activeItem, item);
			if (options.updateFirstItem) updateFirstActiveItem(item);
		},
		[updateFirstActiveItem, updateItem]
	);

	const getNewActiveItemIndex = useCallback(() => {
		const [item] = explorer.selectedItems;

		const uniqueId = item && explorer.getItemUniqueId(item);
		if (!uniqueId) return;

		return explorer.itemsMap.get(uniqueId)?.index;

		// No need to include the whole explorer object here
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [explorer.selectedItems, explorer.itemsMap, explorer.getItemUniqueId]);

	const getItemIndex = useCallback(
		(activeItem: MutableRefObject<ActiveItem>) => {
			if (!activeItem.current) return;
			return explorer.itemsMap.get(activeItem.current)?.index;
		},
		[explorer.itemsMap]
	);

	const getActiveItemIndex = useCallback(
		() => getItemIndex(activeItem) ?? getNewActiveItemIndex(),
		[getItemIndex, getNewActiveItemIndex]
	);

	const getFirstActiveItemIndex = useCallback(
		() => getItemIndex(firstActiveItem),
		[getItemIndex]
	);

	const handleWindowsGridShiftSelection = useCallback(
		(newIndex: number) => {
			if (!explorer.items) return;

			const newItem = explorer.items[newIndex];
			if (!newItem) return;

			const activeItemIndex = getActiveItemIndex() ?? 0;
			const firstActiveItemIndex = getFirstActiveItemIndex() ?? activeItemIndex;

			const item = explorer.items[firstActiveItemIndex];
			if (!item) return;

			const items = explorer.items.slice(
				Math.min(firstActiveItemIndex, newIndex),
				Math.max(firstActiveItemIndex, newIndex) + 1
			);

			explorer.resetSelectedItems(items);

			updateActiveItem(explorer.getItemUniqueId(newItem));
			updateFirstActiveItem(explorer.getItemUniqueId(item));
		},

		// No need to include the whole explorer object here
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[
			explorer.items,
			explorer.getItemUniqueId,
			explorer.resetSelectedItems,
			getActiveItemIndex,
			getFirstActiveItemIndex,
			updateActiveItem,
			updateFirstActiveItem
		]
	);

	return {
		getActiveItemIndex,
		getFirstActiveItemIndex,
		updateActiveItem,
		updateFirstActiveItem,
		handleWindowsGridShiftSelection
	};
}
