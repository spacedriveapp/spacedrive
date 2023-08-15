import { useMemo } from 'react';
import { ExplorerItem, FilePathSearchOrdering, getExplorerItemData } from '@sd/client';
import { ExplorerParamsSchema } from '~/app/route-schemas';
import { useZodSearchParams } from '~/hooks';
import { flattenThumbnailKey, useExplorerStore } from './store';
import { ExplorerItemHash } from './useExplorer';

export function useExplorerOrder(): FilePathSearchOrdering | undefined {
	const explorerStore = useExplorerStore();

	const ordering = useMemo(() => {
		if (explorerStore.orderBy === 'none') return undefined;

		const obj = {};

		explorerStore.orderBy.split('.').reduce((acc, next, i, all) => {
			if (all.length - 1 === i) acc[next] = explorerStore.orderByDirection;
			else acc[next] = {};

			return acc[next];
		}, obj as any);

		return obj as FilePathSearchOrdering;
	}, [explorerStore.orderBy, explorerStore.orderByDirection]);

	return ordering;
}

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
