import { useMemo } from 'react';
import { ExplorerItem } from '@sd/client';
import { getExplorerItemData, getItemFilePath } from '~/app/$libraryId/Explorer/util';
import { useExplorerStore } from './useExplorerStore';

export function useExplorerItemData(explorerItem: ExplorerItem) {
	const filePath = getItemFilePath(explorerItem);
	const { newThumbnails } = useExplorerStore();

	const newThumbnail = newThumbnails?.[filePath?.cas_id || ''] || false;
	return useMemo(() => {
		const itemData = getExplorerItemData(explorerItem);
		if (!itemData.hasThumbnail) {
			itemData.hasThumbnail = newThumbnail;
		}
		return itemData;
	}, [explorerItem, newThumbnail]);
}
