import { ExplorerItem } from '../core';
import { byteSize } from '../lib';
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

export function getExplorerItemData(data?: null | ExplorerItem) {
	const itemData = {
		name: null as string | null,
		size: byteSize(0),
		kind:
			ObjectKind[(data && getItemObject(data)?.kind) || ObjectKind.Unknown] ??
			ObjectKind[ObjectKind.Unknown],
		casId: null as string | null,
		isDir: false,
		extension: null as string | null,
		locationId: null as number | null,
		dateIndexed: null as string | null,
		dateCreated: data?.item.date_created ?? null,
		thumbnailKey: data?.thumbnail_key ?? [],
		hasLocalThumbnail: data?.has_local_thumbnail ?? false // this will be overwritten if new thumbnail is generated
	};

	if (!data) return itemData;

	const filePath = getItemFilePath(data);
	const location = getItemLocation(data);
	if (filePath) {
		itemData.name = filePath.name;
		itemData.size = byteSize(filePath.size_in_bytes_bytes);
		itemData.isDir = filePath.is_dir ?? false;
		itemData.extension = filePath.extension;
		if ('kind' in filePath) itemData.kind = ObjectKind[filePath.kind] as ObjectKindKey;
		if ('cas_id' in filePath) itemData.casId = filePath.cas_id;
		if ('location_id' in filePath) itemData.locationId = filePath.location_id;
		if ('date_indexed' in filePath) itemData.dateIndexed = filePath.date_indexed;
	} else if (location) {
		if (location.total_capacity != null && location.available_capacity != null)
			itemData.size = byteSize(location.total_capacity - location.available_capacity);

		itemData.name = location.name;
		itemData.kind = ObjectKind[ObjectKind.Folder] as ObjectKindKey;
		itemData.isDir = true;
		itemData.locationId = location.id;
		itemData.dateIndexed = location.date_created;
	}

	return itemData;
}
