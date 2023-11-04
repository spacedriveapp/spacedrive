import { useKeys } from 'rooks';
import { useItemsAsEphemeralPaths, useItemsAsFilePaths, useLibraryMutation } from '@sd/client';
import { toast } from '@sd/ui';
import { useExplorerContext } from '~/app/$libraryId/Explorer/Context';
import { getExplorerStore, useExplorerStore } from '~/app/$libraryId/Explorer/store';
import { useExplorerSearchParams } from '~/app/$libraryId/Explorer/util';
import { isNonEmpty } from '~/util';

import { useKeyMatcher } from './useKeyMatcher';

export const useKeyCopyCutPaste = () => {
	const { cutCopyState } = useExplorerStore();
	const [{ path }] = useExplorerSearchParams();

	const metaCtrlKey = useKeyMatcher('Meta').key;
	const copyFiles = useLibraryMutation('files.copyFiles');
	const copyEphemeralFiles = useLibraryMutation('ephemeralFiles.copyFiles');
	const cutFiles = useLibraryMutation('files.cutFiles');
	const cutEphemeralFiles = useLibraryMutation('ephemeralFiles.cutFiles');
	const explorer = useExplorerContext();

	const { parent } = explorer;

	const selectedFilePaths = useItemsAsFilePaths(Array.from(explorer.selectedItems));
	const selectedEphemeralPaths = useItemsAsEphemeralPaths(Array.from(explorer.selectedItems));

	const indexedArgs =
		parent?.type === 'Location' && !isNonEmpty(selectedFilePaths)
			? {
					sourceLocationId: parent.location.id,
					sourcePathIds: selectedFilePaths.map((p) => p.id)
			  }
			: undefined;

	const ephemeralArgs =
		parent?.type === 'Ephemeral' && !isNonEmpty(selectedEphemeralPaths)
			? { sourcePaths: selectedEphemeralPaths.map((p) => p.path) }
			: undefined;

	useKeys([metaCtrlKey, 'KeyC'], (e) => {
		e.stopPropagation();
		if (explorer.parent?.type === 'Location') {
			getExplorerStore().cutCopyState = {
				sourceParentPath: path ?? '/',
				indexedArgs,
				ephemeralArgs,
				type: 'Copy'
			};
		}
	});

	useKeys([metaCtrlKey, 'KeyX'], (e) => {
		e.stopPropagation();
		if (explorer.parent?.type === 'Location') {
			getExplorerStore().cutCopyState = {
				sourceParentPath: path ?? '/',
				indexedArgs,
				ephemeralArgs,
				type: 'Cut'
			};
		}
	});

	useKeys([metaCtrlKey, 'KeyV'], async (e) => {
		e.stopPropagation();
		const parent = explorer.parent;
		if (
			(parent?.type === 'Location' || parent?.type === 'Ephemeral') &&
			cutCopyState.type !== 'Idle' &&
			path
		) {
			const { type, sourceParentPath, indexedArgs, ephemeralArgs } = cutCopyState;

			try {
				if (type == 'Copy') {
					if (parent?.type === 'Location' && indexedArgs != undefined) {
						await copyFiles.mutateAsync({
							source_location_id: indexedArgs.sourceLocationId,
							sources_file_path_ids: [...indexedArgs.sourcePathIds],
							target_location_id: parent.location.id,
							target_location_relative_directory_path: path
						});
					}

					if (parent?.type === 'Ephemeral' && ephemeralArgs != undefined) {
						await copyEphemeralFiles.mutateAsync({
							sources: [...ephemeralArgs.sourcePaths],
							target_dir: path
						});
					}
				} else {
					if (parent?.type === 'Location' && indexedArgs != undefined) {
						if (
							indexedArgs.sourceLocationId === parent.location.id &&
							sourceParentPath === path
						) {
							toast.error('File already exists in this location');
						}
						await cutFiles.mutateAsync({
							source_location_id: indexedArgs.sourceLocationId,
							sources_file_path_ids: [...indexedArgs.sourcePathIds],
							target_location_id: parent.location.id,
							target_location_relative_directory_path: path
						});
					}

					if (parent?.type === 'Ephemeral' && ephemeralArgs != undefined) {
						if (sourceParentPath !== path) {
							await cutEphemeralFiles.mutateAsync({
								sources: [...ephemeralArgs.sourcePaths],
								target_dir: path
							});
						}
					}
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
