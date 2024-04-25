import { useMemo } from 'react';
import { getExplorerItemData, useSelector, type ExplorerItem } from '@sd/client';
import { ExplorerParamsSchema } from '~/app/route-schemas';
import { useZodSearchParams } from '~/hooks';

import { explorerStore, flattenThumbnailKey } from './store';

export function useExplorerSearchParams() {
	return useZodSearchParams(ExplorerParamsSchema);
}

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
	}, [explorerItem, newThumbnail]);
}

export type ExplorerItemData = ReturnType<typeof useExplorerItemData>;

export const pubIdToString = (pub_id: number[]) =>
	pub_id.map((b) => b.toString(16).padStart(2, '0')).join('');

export const uniqueId = (item: ExplorerItem | { pub_id: number[] }) => {
	if ('pub_id' in item) return pubIdToString(item.pub_id);

	const { type } = item;

	switch (type) {
		case 'NonIndexedPath':
			return item.item.path;
		case 'SpacedropPeer':
		case 'Label':
			return item.item.name;
		default:
			return pubIdToString(item.item.pub_id);
	}
};

export function getItemId(index: number, items: ExplorerItem[]) {
	const item = items[index];
	return item ? uniqueId(item) : undefined;
}

export function getItemData(index: number, items: ExplorerItem[]) {
	return items[index];
}
