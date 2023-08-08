import { useMemo } from 'react';
import { ExplorerItem, FilePath } from '@sd/client';

export const useItemsAsFilePaths = (items: ExplorerItem[]) => {
	return useMemo(() => {
		const array: FilePath[] = [];

		for (const item of items) {
			switch (item.type) {
				case 'Path': {
					array.push(item.item);
					break;
				}
				case 'Object': {
					// this isn't good but it's the current behaviour
					const filePath = item.item.file_paths[0];
					if (filePath) array.push(filePath);
					else return [];

					break;
				}
				default:
					return [];
			}
		}

		return array;
	}, [items]);
};
