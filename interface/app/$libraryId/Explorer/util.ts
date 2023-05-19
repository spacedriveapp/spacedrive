import { z } from 'zod';
import { ExplorerItem, ObjectKind, ObjectKindKey, Ordering, isObject, isPath } from '@sd/client';
import { useExplorerStore, useZodSearchParams } from '~/hooks';

export function getExplorerItemData(data: ExplorerItem, hasNewThumbnail?: boolean) {
	const objectData = getItemObject(data);
	const filePath = getItemFilePath(data);

	return {
		cas_id: filePath?.cas_id || null,
		isDir: isPath(data) && data.item.is_dir,
		kind: (ObjectKind[objectData?.kind ?? 0] as ObjectKindKey) || null,
		hasThumbnail: data.has_thumbnail || hasNewThumbnail,
		extension: filePath?.extension || null
	};
}

export function useExplorerOrder(): Ordering | undefined {
	const explorerStore = useExplorerStore();

	if (explorerStore.orderBy === 'none') return undefined;

	return { [explorerStore.orderBy]: explorerStore.orderByDirection === 'asc' } as Ordering;
}

export function getItemObject(data: ExplorerItem) {
	return isObject(data) ? data.item : data.item.object;
}

export function getItemFilePath(data: ExplorerItem) {
	return isObject(data) ? data.item.file_paths[0] : data.item;
}

export const SEARCH_PARAMS = z.object({
	path: z.string().optional(),
	take: z.coerce.number().default(100)
});

export function useExplorerSearchParams() {
	return useZodSearchParams(SEARCH_PARAMS);
}
