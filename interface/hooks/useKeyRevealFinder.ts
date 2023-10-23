import {useLibraryContext} from '@sd/client';
import { usePlatform, type Platform } from '~/util/Platform';
import { useMemo } from 'react';
import { useKeysMatcher } from './useKeyMatcher';
import { useKeys } from 'rooks';
import { useExplorerContext } from '~/app/$libraryId/Explorer/Context';

export const useKeyRevealFinder = () => {
	const explorer = useExplorerContext();
	const {revealItems} = usePlatform();
	const shortcuts = useKeysMatcher(['Meta'])
	const {library} = useLibraryContext();

	const items = useMemo(() => {
		const array: Parameters<NonNullable<Platform['revealItems']>>[1] = [];

		for (const item of explorer.selectedItems.values()) {
			switch (item.type) {
				case 'Path': {
					array.push({
						FilePath: { id: item.item.id }
					});
					break;
				}
				case 'Object': {
					// this isn't good but it's the current behaviour
					const filePath = item.item.file_paths[0];
					if (filePath)
						array.push({
							FilePath: {
								id: filePath.id
							}
						});
					else return [];
					break;
				}
				case 'Location': {
					array.push({
						Location: {
							id: item.item.id
						}
					});
					break;
				}
				case 'NonIndexedPath': {
					array.push({
						Ephemeral: {
							path: item.item.path
						}
					});
					break;
				}
			}
		}
		return array;
	}, [explorer.selectedItems]);

	useKeys([shortcuts.Meta.key, 'KeyY'], (e) => {
		e.stopPropagation();
		if (!revealItems) return;
			revealItems(
				library.uuid,
				items
			);
	});
}
