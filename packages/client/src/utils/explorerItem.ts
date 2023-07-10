import { ExplorerItem } from '../core';
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
