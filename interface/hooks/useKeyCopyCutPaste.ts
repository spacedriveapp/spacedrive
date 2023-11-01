import { useKeys } from 'rooks';
import { useItemsAsFilePaths, useLibraryMutation } from '@sd/client';
import { toast } from '@sd/ui';
import { useExplorerContext } from '~/app/$libraryId/Explorer/Context';
import { getExplorerStore, useExplorerStore } from '~/app/$libraryId/Explorer/store';
import { useExplorerSearchParams } from '~/app/$libraryId/Explorer/util';

import { useShortcut } from './useShortcut';

export const useKeyCopyCutPaste = () => {
	const { cutCopyState } = useExplorerStore();
	const [{ path }] = useExplorerSearchParams();
	const shortcut = useShortcut();

	const copyFiles = useLibraryMutation('files.copyFiles');
	const cutFiles = useLibraryMutation('files.cutFiles');
	const explorer = useExplorerContext();
	const selectedFilePaths = useItemsAsFilePaths(Array.from(explorer.selectedItems));
	const parent = explorer.parent;

	useKeys(shortcut.copyObject, (e) => {
		e.stopPropagation();
		if (explorer.parent?.type === 'Location') {
			getExplorerStore().cutCopyState = {
				sourceParentPath: path ?? '/',
				sourceLocationId: explorer.parent.location.id,
				sourcePathIds: selectedFilePaths.map((p) => p.id),
				type: 'Copy'
			};
		}
	});

	useKeys(shortcut.cutObject, (e) => {
		e.stopPropagation();
		if (explorer.parent?.type === 'Location') {
			getExplorerStore().cutCopyState = {
				sourceParentPath: path ?? '/',
				sourceLocationId: explorer.parent.location.id,
				sourcePathIds: selectedFilePaths.map((p) => p.id),
				type: 'Cut'
			};
		}
	});

	useKeys(shortcut.duplicateObject, async (e) => {
		e.stopPropagation();
		if (parent?.type === 'Location') {
			console.log('hello');
			try {
				await copyFiles.mutateAsync({
					source_location_id: parent.location.id,
					sources_file_path_ids: selectedFilePaths.map((p) => p.id),
					target_location_id: parent.location.id,
					target_location_relative_directory_path: path ?? '/'
				});
			} catch (error) {
				toast.error({
					title: 'Failed to duplicate file',
					body: `Error: ${error}.`
				});
			}
		}
	});

	useKeys(shortcut.pasteObject, async (e) => {
		e.stopPropagation();
		if (parent?.type === 'Location' && cutCopyState.type !== 'Idle' && path) {
			const { type, sourcePathIds, sourceParentPath, sourceLocationId } = cutCopyState;

			const sameLocation =
				sourceLocationId === parent.location.id && sourceParentPath === path;

			try {
				if (type == 'Copy') {
					await copyFiles.mutateAsync({
						source_location_id: sourceLocationId,
						sources_file_path_ids: [...sourcePathIds],
						target_location_id: parent.location.id,
						target_location_relative_directory_path: path
					});
				} else if (sameLocation) {
					toast.error('File already exists in this location');
				} else {
					await cutFiles.mutateAsync({
						source_location_id: sourceLocationId,
						sources_file_path_ids: [...sourcePathIds],
						target_location_id: parent.location.id,
						target_location_relative_directory_path: path
					});
				}
			} catch (error) {
				toast.error({
					title: `Failed to ${type.toLowerCase()} file`,
					body: `Error: ${error}.`
				});
			}
		}
	});
};
