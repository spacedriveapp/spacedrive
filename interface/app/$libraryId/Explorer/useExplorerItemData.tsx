import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo } from 'react';
import { getExplorerItemData, useSelector, type ExplorerItem } from '@sd/client';

import { explorerStore, flattenThumbnailKey } from './store';

// This is where we intercept the state of the explorer item to determine if we should rerender
// This hook is used inside every thumbnail in the explorer
export function useExplorerItemData(explorerItem: ExplorerItem) {
	const newThumbnail = useSelector(explorerStore, (s) => {
		const thumbnailKey =
			explorerItem.type === 'Label'
				? // labels have .thumbnails, plural
					explorerItem.thumbnails?.[0]
				: // all other explorer items have .thumbnail singular
					'thumbnail' in explorerItem && explorerItem.thumbnail;

		return !!(thumbnailKey && s.newThumbnails.has(flattenThumbnailKey(thumbnailKey)));
	});

	return useMemo(() => {
		const itemData = getExplorerItemData(explorerItem);

		if (!itemData.hasLocalThumbnail) {
			itemData.hasLocalThumbnail = newThumbnail;
		}

		return itemData;
		// whatever goes here, is what can cause an atomic re-render of an explorer item
		// this is used for when new thumbnails are generated, and files identified
	}, [explorerItem, newThumbnail]);
}

export type ExplorerItemData = ReturnType<typeof useExplorerItemData>;
