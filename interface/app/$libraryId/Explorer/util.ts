import { z } from 'zod';
import {
	ExplorerItem,
	FilePathSearchOrdering,
	ObjectKind,
	ObjectKindKey,
	isObject,
	isPath
} from '@sd/client';
import { useExplorerStore, useZodSearchParams } from '~/hooks';
import { useMemo } from 'react';

export function useExplorerOrder(): FilePathSearchOrdering | undefined {
	const explorerStore = useExplorerStore();

	const ordering = useMemo(() => {
		if (explorerStore.orderBy === 'none') return undefined;

		const obj = {};

		explorerStore.orderBy.split('.').reduce((acc, next, i, all) => {
			if(all.length - 1 === i) acc[next] = explorerStore.orderByDirection;
			else acc[next] = {}

			return acc[next]
		}, obj as any)

		return obj as FilePathSearchOrdering;
	}, [explorerStore.orderBy, explorerStore.orderByDirection])

	return ordering
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
