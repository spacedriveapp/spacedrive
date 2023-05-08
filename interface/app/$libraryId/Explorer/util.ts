import { z } from 'zod';
import { ExplorerItem, ObjectKind, ObjectKindKey, isObject, isPath } from '@sd/client';
import { useZodSearchParams } from '~/hooks';

export function getExplorerItemData(data: ExplorerItem, newThumbnails?: Record<string, boolean>) {
	const objectData = getItemObject(data);
	const filePath = getItemFilePath(data);

	return {
		cas_id: filePath?.cas_id || null,
		isDir: isPath(data) && data.item.is_dir,
		kind: (ObjectKind[objectData?.kind ?? 0] as ObjectKindKey) || null,
		newThumb :!!newThumbnails?.[filePath?.cas_id || ''],
		hasThumbnail: data.has_thumbnail || !!newThumbnails?.[filePath?.cas_id || ''] || false,
		extension: filePath?.extension || null
	};
}

export function getItemObject(data: ExplorerItem) {
	return isObject(data) ? data.item : data.item.object;
}

export function getItemFilePath(data: ExplorerItem) {
	return isObject(data) ? data.item.file_paths[0] : data.item;
}

export const SEARCH_PARAMS = z.object({
	path: z.string().default(''),
	limit: z.coerce.number().default(100)
});

export function useExplorerSearchParams() {
	return useZodSearchParams(SEARCH_PARAMS);
}
