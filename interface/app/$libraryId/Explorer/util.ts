import { ExplorerItem, ObjectKind, ObjectKinds, isObject, isPath } from '@sd/client';

export function getExplorerItemData(data: ExplorerItem) {
	const objectData = getItemObject(data);
	const filePath = getItemFilePath(data);

	return {
		cas_id: filePath?.cas_id || null,
		isDir: isPath(data) && data.item.is_dir,
		kind: (ObjectKind[objectData?.kind ?? 0] as ObjectKinds) || null,
		hasThumbnail: data.has_thumbnail,
		extension: filePath?.extension || null
	};
}

export function getItemObject(data: ExplorerItem) {
	return isObject(data) ? data.item : data.item.object;
}

export function getItemFilePath(data: ExplorerItem) {
	return isObject(data) ? data.item.file_paths[0] : data.item;
}
