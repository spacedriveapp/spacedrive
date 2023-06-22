import { useMemo } from 'react';
import { ExplorerItem, getExplorerItemData } from '@sd/client';
import { flattenThumbnailKey, useExplorerStore } from './useExplorerStore';

export function useExplorerItemData(explorerItem: ExplorerItem) {
	const explorerStore = useExplorerStore();

	const newThumbnail = !!(
		explorerItem.thumbnail_key &&
		explorerStore.newThumbnails.has(flattenThumbnailKey(explorerItem.thumbnail_key))
	);

	return useMemo(() => {
		const itemData = getExplorerItemData(explorerItem);

		if (!itemData.hasLocalThumbnail) {
			itemData.hasLocalThumbnail = newThumbnail;
		}

		return itemData;
	}, [explorerItem, newThumbnail]);
}
