import { ExplorerItem, ObjectKind, isObject, isPath } from '@sd/client';

export function getExplorerItemData(data: ExplorerItem) {
	const objectData = data ? (isObject(data) ? data.item : data.item.object) : null;

	return {
		cas_id: (isObject(data) ? data.item.file_paths[0]?.cas_id : data.item.cas_id) || null,
		isDir: isPath(data) && data.item.is_dir,
		kind: ObjectKind[objectData?.kind || 0] || null,
		hasThumbnail: data.has_thumbnail,
		extension: data.item.extension
	};
}
