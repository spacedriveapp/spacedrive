import {
	useItemsAsEphemeralPaths,
	useItemsAsFilePaths,
	useLibraryMutation,
	useSelector
} from '@sd/client';
import { toast } from '@sd/ui';
import { useExplorerContext } from '~/app/$libraryId/Explorer/Context';
import { explorerStore } from '~/app/$libraryId/Explorer/store';
import { useExplorerSearchParams } from '~/app/$libraryId/Explorer/util';

import { useLocale } from './useLocale';
import { useShortcut } from './useShortcut';

export const useKeyCopyCutPaste = () => {
	const { t } = useLocale();

	const cutCopyState = useSelector(explorerStore, (s) => s.cutCopyState);
	const [{ path }] = useExplorerSearchParams();

	const copyFiles = useLibraryMutation('files.copyFiles');
	const copyEphemeralFiles = useLibraryMutation('ephemeralFiles.copyFiles');
	const cutFiles = useLibraryMutation('files.cutFiles');
	const cutEphemeralFiles = useLibraryMutation('ephemeralFiles.cutFiles');
	const explorer = useExplorerContext();

	const { parent } = explorer;

	const selectedFilePaths = useItemsAsFilePaths(Array.from(explorer.selectedItems));
	const selectedEphemeralPaths = useItemsAsEphemeralPaths(Array.from(explorer.selectedItems));

	const indexedArgs =
		parent?.type === 'Location'
			? {
					sourceLocationId: parent.location.id,
					sourcePathIds: selectedFilePaths.map((p) => p.id)
			  }
			: undefined;

	const ephemeralArgs =
		parent?.type === 'Ephemeral'
			? { sourcePaths: selectedEphemeralPaths.map((p) => p.path) }
			: undefined;

	useShortcut('copyObject', (e) => {
		e.stopPropagation();
		if (explorer.parent?.type === 'Location') {
			explorerStore.cutCopyState = {
				sourceParentPath: path ?? '/',
				type: 'Copy',
				indexedArgs,
				ephemeralArgs
			};
		}
	});

	useShortcut('cutObject', (e) => {
		e.stopPropagation();
		if (explorer.parent?.type === 'Location') {
			explorerStore.cutCopyState = {
				sourceParentPath: path ?? '/',
				type: 'Cut',
				indexedArgs,
				ephemeralArgs
			};
		}
	});

	useShortcut('duplicateObject', async (e) => {
		e.stopPropagation();
		if (parent?.type === 'Location') {
			try {
				await copyFiles.mutateAsync({
					source_location_id: parent.location.id,
					sources_file_path_ids: selectedFilePaths.map((p) => p.id),
					target_location_id: parent.location.id,
					target_location_relative_directory_path: path ?? '/'
				});
			} catch (error) {
				toast.error({
					title: t('failed_to_duplicate_file'),
					body: `Error: ${error}.`
				});
			}
		}
	});

	useShortcut('pasteObject', async (e) => {
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
							toast.error(t('file_already_exist_in_this_location'));
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
				if (type === 'Copy') {
					toast.error({
						title: t('failed_to_copy_file'),
						body: `Error: ${error}.`
					});
				} else if (type === 'Cut') {
					toast.error({
						title: t('failed_to_cut_file'),
						body: `Error: ${error}.`
					});
				}
			}
		}
	});
};
