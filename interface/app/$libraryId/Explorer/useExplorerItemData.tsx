import type { ExplorerItem } from '@sd/client';

import { useCallback, useMemo, useRef } from 'react';

import {
	compareHumanizedSizes,
	getExplorerItemData,
	humanizeSize,
	ThumbKey,
	useSelector
} from '@sd/client';
import { usePlatform } from '~/util/Platform';

import { explorerStore, flattenThumbnailKey } from './store';

/**
 * This is where we intercept the state of the explorer item to determine if we should rerender
 *
 * .. WARNING::
 *    This hook is used inside every thumbnail in the explorer.
 * 	  Be careful with the performance of the code, make sure to always memoize any objects or functions to avoid unnecessary re-renders.
 *
 * @param explorerItem - The explorer item to get data from
 * @returns The extracted data from the explorer item
 */
export function useExplorerItemData(explorerItem: ExplorerItem) {
	const platform = usePlatform();
	const cachedSize = useRef<ReturnType<typeof humanizeSize> | null>(null);
	const getThumbnails = useCallback(
		() =>
			new Map(
				(explorerItem.type === 'Label'
					? explorerItem.thumbnails
					: 'thumbnail' in explorerItem && explorerItem.thumbnail
						? [explorerItem.thumbnail]
						: []
				).map<[string, ThumbKey]>(thumbnailKey => [
					platform.getThumbnailUrlByThumbKey(thumbnailKey),
					thumbnailKey
				])
			),
		[explorerItem, platform]
	);

	const newThumbnails = useSelector(explorerStore, store =>
		Array.from(getThumbnails()).reduce<Map<string, string | null>>((acc, [url, thumbKey]) => {
			const thumbId = flattenThumbnailKey(thumbKey);
			acc.set(url, store.newThumbnails.has(thumbId) ? thumbId : null);
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

	return useMemo(() => {
		const explorerItemData = getExplorerItemData(explorerItem);

		// Avoid unecessary re-renders
		if (
			cachedSize.current == null ||
			!compareHumanizedSizes(cachedSize.current, explorerItemData.size)
		) {
			cachedSize.current = explorerItemData.size;
		}

		return {
			...explorerItemData,
			size: cachedSize.current,
			thumbnails: newThumbnails
		};
	}, [explorerItem, newThumbnails]);
}

export type ExplorerItemData = ReturnType<typeof useExplorerItemData>;
