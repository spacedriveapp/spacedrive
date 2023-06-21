import { useMemo } from 'react';
import { z } from 'zod';
import { ExplorerItem, FilePathSearchOrdering, ObjectKind, ObjectKindKey } from '@sd/client';
import { useExplorerStore, useZodSearchParams } from '~/hooks';

export function useExplorerOrder(): FilePathSearchOrdering | undefined {
	const explorerStore = useExplorerStore();

	const ordering = useMemo(() => {
		if (explorerStore.orderBy === 'none') return undefined;

		const obj = {};

		explorerStore.orderBy.split('.').reduce((acc, next, i, all) => {
			if (all.length - 1 === i) acc[next] = explorerStore.orderByDirection;
			else acc[next] = {};

			return acc[next];
		}, obj as any);

		return obj as FilePathSearchOrdering;
	}, [explorerStore.orderBy, explorerStore.orderByDirection]);

	return ordering;
}

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

export const SEARCH_PARAMS = z.object({
	path: z.string().optional(),
	take: z.coerce.number().default(100)
});

export function useExplorerSearchParams() {
	return useZodSearchParams(SEARCH_PARAMS);
}
