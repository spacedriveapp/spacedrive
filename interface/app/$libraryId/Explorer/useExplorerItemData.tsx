import { useCallback, useMemo } from 'react';
import { getExplorerItemData, ThumbKey, useSelector, type ExplorerItem } from '@sd/client';
import { usePlatform } from '~/util/Platform';

import { explorerStore, flattenThumbnailKey } from './store';

// This is where we intercept the state of the explorer item to determine if we should rerender
// This hook is used inside every thumbnail in the explorer
export function useExplorerItemData(explorerItem: ExplorerItem) {
	const platform = usePlatform();
	const getThumbnails = useCallback(
		() =>
			new Map(
				(explorerItem.type === 'Label'
					? explorerItem.thumbnails
					: 'thumbnail' in explorerItem && explorerItem.thumbnail
						? [explorerItem.thumbnail]
						: []
				).map<[ThumbKey, string]>((thumbnailKey) => [
					thumbnailKey,
					platform.getThumbnailUrlByThumbKey(thumbnailKey)
				])
			),
		[explorerItem, platform]
	);

	const newThumbnails = useSelector(explorerStore, (store) =>
		Array.from(getThumbnails()).reduce<Map<string, string | null>>((acc, [key, thumbnail]) => {
			const thumbId = flattenThumbnailKey(key);
			acc.set(thumbnail, store.newThumbnails.has(thumbId) ? thumbId : null);
			return acc;
		}, new Map())
	);

	if (
		'has_created_thumbnail' in explorerItem &&
		explorerItem.has_created_thumbnail &&
		newThumbnails.size === 0
	) {
		console.warn('ExplorerItem has created thumbnail but no new thumbnail found', explorerItem);
	}

	return useMemo(
		// whatever goes here, is what can cause an atomic re-render of an explorer item
		// this is used for when new thumbnails are generated, and files identified
		() => ({
			...getExplorerItemData(explorerItem),
			thumbnails: newThumbnails
		}),
		[explorerItem, newThumbnails]
	);
}

export type ExplorerItemData = ReturnType<typeof useExplorerItemData>;
