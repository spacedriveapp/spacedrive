import { z } from 'zod';
import { ExplorerItem, ObjectKind, ObjectKindKey, Ordering, isObject, isPath } from '@sd/client';
import { useExplorerStore, useZodSearchParams } from '~/hooks';

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

export function getExplorerItemData(data: ExplorerItem) {
	const filePath = getItemFilePath(data);
	const objectData = getItemObject(data);

	return {
		kind: (ObjectKind[objectData?.kind ?? 0] as ObjectKindKey) || null,
		casId: filePath?.cas_id || null,
		isDir: isPath(data) && data.item.is_dir,
		extension: filePath?.extension || null,
		hasThumbnail: data.has_thumbnail
	};
}

export const SEARCH_PARAMS = z.object({
	path: z.string().optional(),
	take: z.coerce.number().default(100)
});

export function useExplorerSearchParams() {
	return useZodSearchParams(SEARCH_PARAMS);
}
