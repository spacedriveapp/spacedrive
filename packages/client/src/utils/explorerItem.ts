import { useMemo } from 'react';

import type { ExplorerItem, FilePath, Object } from '../core';
import { byteSize } from '../lib';
import { ObjectKind } from './objectKind';

export function getItemObject(data: ExplorerItem) {
	return data.type === 'Object' ? data.item : data.type === 'Path' ? data.item.object : null;
}

export function getItemFilePath(data: ExplorerItem) {
	if (data.type === 'Path' || data.type === 'NonIndexedPath') return data.item;
	return (data.type === 'Object' && data.item.file_paths[0]) || null;
}

export function getIndexedItemFilePath(data: ExplorerItem) {
	return data.type === 'Path'
		? data.item
		: data.type === 'Object'
		? data.item.file_paths[0] ?? null
		: null;
}

export function getItemLocation(data: ExplorerItem) {
	return data.type === 'Location' ? data.item : null;
}
export function getItemSpacedropPeer(data: ExplorerItem) {
	return data.type === 'SpacedropPeer' ? data.item : null;
}

export function getExplorerItemData(data?: null | ExplorerItem) {
	const itemObj = data ? getItemObject(data) : null;

	const kind = (itemObj?.kind ? ObjectKind[itemObj.kind] : null) ?? 'Unknown';

	const itemData = {
		name: null as string | null,
		fullName: null as string | null,
		size: byteSize(0),
		kind,
		isDir: false,
		casId: null as string | null,
		extension: null as string | null,
		locationId: null as number | null,
		dateIndexed: null as string | null,
		dateCreated: data?.item.date_created ?? itemObj?.date_created ?? null,
		dateModified: null as string | null,
		dateAccessed: itemObj?.date_accessed ?? null,
		thumbnailKey: data?.thumbnail_key ?? [],
		hasLocalThumbnail: data?.has_local_thumbnail ?? false, // this will be overwritten if new thumbnail is generated
		customIcon: null as string | null
	};

	if (!data) return itemData;

	const filePath = getItemFilePath(data);
	const location = getItemLocation(data);
	if (filePath) {
		itemData.name = filePath.name;
		itemData.fullName = `${filePath.name}${filePath.extension ? `.${filePath.extension}` : ''}`;
		itemData.size = byteSize(filePath.size_in_bytes_bytes);
		itemData.isDir = filePath.is_dir ?? false;
		itemData.extension = filePath.extension;
		if ('kind' in filePath) itemData.kind = ObjectKind[filePath.kind] ?? 'Unknown';
		if ('cas_id' in filePath) itemData.casId = filePath.cas_id;
		if ('location_id' in filePath) itemData.locationId = filePath.location_id;
		if ('date_indexed' in filePath) itemData.dateIndexed = filePath.date_indexed;
		if ('date_modified' in filePath) itemData.dateModified = filePath.date_modified;
	} else if (location) {
		if (location.total_capacity != null && location.available_capacity != null)
			itemData.size = byteSize(location.total_capacity - location.available_capacity);

		itemData.name = location.name;
		itemData.fullName = location.name;
		itemData.kind = ObjectKind[ObjectKind.Folder] ?? 'Unknown';
		itemData.isDir = true;
		itemData.locationId = location.id;
		itemData.dateIndexed = location.date_created;
	} else if (data.type === 'SpacedropPeer') {
		itemData.name = data.item.name;
		itemData.customIcon = 'Laptop';
	}

	if (data.type == 'Path' && itemData.isDir) itemData.kind = 'Folder';

	return itemData;
}

export const useItemsAsObjects = (items: ExplorerItem[]) => {
	return useMemo(() => {
		const array: Object[] = [];

		for (const item of items) {
			switch (item.type) {
				case 'Path': {
					if (!item.item.object) return [];
					array.push(item.item.object);
					break;
				}
				case 'Object': {
					array.push(item.item);
					break;
				}
				default:
					return [];
			}
		}

		return array;
	}, [items]);
};

export const useItemsAsFilePaths = (items: ExplorerItem[]) => {
	return useMemo(() => {
		const array: FilePath[] = [];

		for (const item of items) {
			switch (item.type) {
				case 'Path': {
					array.push(item.item);
					break;
				}
				case 'Object': {
					// this isn't good but it's the current behaviour
					const filePath = item.item.file_paths[0];
					if (filePath) array.push(filePath);
					else return [];

					break;
				}
				default:
					return [];
			}
		}

		return array;
	}, [items]);
};

export const useItemsAsEphemeralPaths = (items: ExplorerItem[]) => {
	return useMemo(() => {
		return items
			.filter((item) => item.type === 'NonIndexedPath')
			.map((item) => item.item as NonIndexedPathItem);
	}, [items]);
};
