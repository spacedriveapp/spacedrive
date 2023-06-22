import { useMemo } from 'react';
import { FilePathSearchOrdering } from '@sd/client';
import { useExplorerStore } from '~/hooks';

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
