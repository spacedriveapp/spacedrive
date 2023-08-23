import { useMemo } from 'react';
import { ExplorerItem, getExplorerItemData } from '@sd/client';
import { ExplorerParamsSchema } from '~/app/route-schemas';
import { useZodSearchParams } from '~/hooks';
import { flattenThumbnailKey, useExplorerStore } from './store';
import { ExplorerItemHash } from './useExplorer';

export function useExplorerSearchParams() {
	return useZodSearchParams(ExplorerParamsSchema);
}

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

export function explorerItemHash(item: ExplorerItem): ExplorerItemHash {
	return `${item.type}:${item.item.id}`;
}
