import { useMemo } from 'react';
import { useSearchParams } from 'react-router-dom';
import { getParams } from 'remix-params-helper';
import { z } from 'zod';
import { ExplorerItem, ObjectKind, ObjectKindKey, isObject, isPath } from '@sd/client';

export function getExplorerItemData(data: ExplorerItem) {
	const objectData = getItemObject(data);
	const filePath = getItemFilePath(data);

	return {
		cas_id: filePath?.cas_id || null,
		isDir: isPath(data) && data.item.is_dir,
		kind: (ObjectKind[objectData?.kind ?? 0] as ObjectKindKey) || null,
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

const SEARCH_PARAMS = z.object({
	path: z.string().optional(),
	limit: z.coerce.number().default(100)
});

export function useExplorerSearchParams() {
	const [searchParams] = useSearchParams();

	const result = useMemo(() => getParams(searchParams, SEARCH_PARAMS), [searchParams]);

	if (!result.success) throw result.errors;

	return result.data;
}
