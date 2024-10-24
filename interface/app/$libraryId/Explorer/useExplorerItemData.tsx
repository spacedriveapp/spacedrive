import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { subscribe } from 'valtio';
import {
	compareHumanizedSizes,
	getExplorerItemData,
	humanizeSize,
	ThumbKey,
	type ExplorerItem
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
	const [newThumbnails, setNewThumbnails] = useState<Map<string, string | null>>(new Map());

	let thumbnails: ThumbKey | ThumbKey[] | null = null;
	switch (explorerItem.type) {
		case 'Label':
			thumbnails = explorerItem.thumbnails;
			break;
		case 'Path':
		case 'Object':
		case 'NonIndexedPath':
			thumbnails = explorerItem.thumbnail;
			break;
	}

	useEffect(() => {
		const thumbnailKeys = thumbnails
			? Array.isArray(thumbnails)
				? thumbnails
				: [thumbnails]
			: [];

		const updateThumbnails = () =>
			setNewThumbnails((oldThumbs) => {
				const thumbs = thumbnailKeys.reduce<Map<string, string | null>>((acc, thumbKey) => {
					const url = platform.getThumbnailUrlByThumbKey(thumbKey);
					const thumbId = flattenThumbnailKey(thumbKey);
					acc.set(url, explorerStore.newThumbnails.has(thumbId) ? thumbId : null);
					return acc;
				}, new Map());

				// Avoid unnecessary re-renders
				return oldThumbs.size !== thumbs.size ||
					Array.from(oldThumbs.keys()).some(
						(key) => oldThumbs.get(key) !== thumbs.get(key)
					)
					? thumbs
					: oldThumbs;
			});

		updateThumbnails();

		return subscribe(explorerStore, updateThumbnails);
	}, [thumbnails, platform]);

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
			thumbnails: newThumbnails,
			hasLocalThumbnail: explorerItemData.hasLocalThumbnail || newThumbnails.size > 0
		};
	}, [explorerItem, newThumbnails]);
}

export type ExplorerItemData = ReturnType<typeof useExplorerItemData>;
