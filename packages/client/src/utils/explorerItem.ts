import { useMemo } from 'react';
import { ExplorerItem, FilePath, Object } from '../core';
import { ObjectKind, ObjectKindKey } from './objectKind';

export function getItemObject(data: ExplorerItem) {
	return data.type === 'Object' ? data.item : data.type === 'Path' ? data.item.object : null;
}

export function getItemFilePath(data: ExplorerItem) {
	return data.type === 'Path'
		? data.item
		: data.type === 'Object'
		? data.item.file_paths[0]
		: null;
}

export function getItemLocation(data: ExplorerItem) {
	return data.type === 'Location' ? data.item : null;
}

export function getExplorerItemData(data: ExplorerItem) {
	const filePath = getItemFilePath(data);
	const objectData = getItemObject(data);

	return {
		kind: (ObjectKind[objectData?.kind ?? 0] as ObjectKindKey) || null,
		casId: filePath?.cas_id || null,
		isDir: getItemFilePath(data)?.is_dir || false,
		extension: filePath?.extension || null,
		locationId: filePath?.location_id || null,
		hasLocalThumbnail: data.has_local_thumbnail, // this will be overwritten if new thumbnail is generated
		thumbnailKey: data.thumbnail_key
	};
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
