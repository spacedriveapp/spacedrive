import { useMemo } from 'react';
import { z } from 'zod';
import { FilePathSearchOrdering } from '@sd/client';
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

export const SEARCH_PARAMS = z.object({
	path: z.string().optional(),
	take: z.coerce.number().default(100)
});

export function useExplorerSearchParams() {
	return useZodSearchParams(SEARCH_PARAMS);
}
