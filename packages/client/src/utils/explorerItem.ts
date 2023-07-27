import { ExplorerItem } from '../core';
import { ObjectKind, ObjectKindKey } from './objectKind';

export function getItemObject(data: ExplorerItem) {
	return data.type === 'Object' ? data.item : data.type === 'Path' ? data.item.object : null;
}

export function getItemFilePath(data: ExplorerItem) {
	if (data.type === 'Path' || data.type === 'NonIndexedPath') return data.item;
	return (data.type === 'Object' && data.item.file_paths[0]) || null;
}

export function getItemLocation(data: ExplorerItem) {
	return data.type === 'Location' ? data.item : null;
}

export function getExplorerItemData(data: ExplorerItem) {
	const filePath = getItemFilePath(data);

	const itemData = {
		kind: (ObjectKind[getItemObject(data)?.kind ?? 0] as ObjectKindKey) ?? null,
		casId: null as string | null,
		isDir: false,
		extension: null as string | null,
		locationId: null as number | null,
		thumbnailKey: data.thumbnail_key,
		hasLocalThumbnail: data.has_local_thumbnail // this will be overwritten if new thumbnail is generated
	};

	if (filePath) {
		itemData.isDir = filePath.is_dir ?? false;
		itemData.extension = filePath.extension;
		if ('kind' in filePath) itemData.kind = ObjectKind[filePath.kind] as ObjectKindKey;
		if ('cas_id' in filePath) itemData.casId = filePath.cas_id;
		if ('location_id' in filePath) itemData.locationId = filePath.location_id;
	}

	return itemData;
}
