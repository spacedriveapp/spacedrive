import { getIndexedItemFilePath, libraryClient, useLibraryMutation } from '@sd/client';
import { toast } from '@sd/ui';
import { useExplorerContext } from '~/app/$libraryId/Explorer/Context';
import { explorerStore } from '~/app/$libraryId/Explorer/store';
import { useExplorerSearchParams } from '~/app/$libraryId/Explorer/util';
import { useLocale } from '~/hooks';

export const useExplorerCopyPaste = () => {
	const { t } = useLocale();

	const explorer = useExplorerContext();
	const [{ path: currentPath }] = useExplorerSearchParams();

	const copyFiles = useLibraryMutation('files.copyFiles');
	const copyEphemeralFiles = useLibraryMutation('ephemeralFiles.copyFiles');
	const cutFiles = useLibraryMutation('files.cutFiles');
	const cutEphemeralFiles = useLibraryMutation('ephemeralFiles.cutFiles');

	const path = currentPath ?? '/';

	function getIndexedArgs() {
		if (explorer.parent?.type !== 'Location') return;

		const filePathIds: number[] = [];

		for (const item of Array.from(explorer.selectedItems)) {
			const filePath = getIndexedItemFilePath(item);
			if (filePath) filePathIds.push(filePath.id);
		}

		return {
			sourceLocationId: explorer.parent.location.id,
			sourcePathIds: filePathIds
		};
	}

	function getEphemeralArgs() {
		if (explorer.parent?.type !== 'Ephemeral') return;

		const sourcePaths: string[] = [];

		for (const item of Array.from(explorer.selectedItems)) {
			const filePath = item.type === 'NonIndexedPath' && item.item.path;
			if (filePath) sourcePaths.push(filePath);
		}

		return { sourcePaths };
	}

	function copy() {
		explorerStore.cutCopyState = {
			type: 'Copy',
			sourceParentPath: path,
			indexedArgs: getIndexedArgs(),
			ephemeralArgs: getEphemeralArgs()
		};
	}

	function cut() {
		explorerStore.cutCopyState = {
			type: 'Cut',
			sourceParentPath: path,
			indexedArgs: getIndexedArgs(),
			ephemeralArgs: getEphemeralArgs()
		};
	}

	async function duplicate() {
		try {
			if (explorer.parent?.type === 'Location') {
				const args = getIndexedArgs();
				if (!args) return;

				await copyFiles.mutateAsync({
					source_location_id: explorer.parent.location.id,
					sources_file_path_ids: args.sourcePathIds,
					target_location_id: explorer.parent.location.id,
					target_location_relative_directory_path: path
				});

				toast.success(t('duplicate_success'));
			}

			if (explorer.parent?.type === 'Ephemeral') {
				const args = getEphemeralArgs();
				if (!args) return;

				await copyEphemeralFiles.mutateAsync({
					sources: args.sourcePaths,
					target_dir: path
				});

				toast.success(t('duplicate_success'));
			}
		} catch (error) {
			toast.error({
				title: t('failed_to_duplicate_file'),
				body: `Error: ${error}.`
			});
		}
	}

	async function paste() {
		if (explorerStore.cutCopyState.type === 'Idle') return;

		const { type, indexedArgs, ephemeralArgs } = explorerStore.cutCopyState;

		try {
			if (ephemeralArgs || (indexedArgs && explorer.parent?.type === 'Ephemeral')) {
				const mutation = type === 'Copy' ? copyEphemeralFiles : cutEphemeralFiles;
				const sources = ephemeralArgs?.sourcePaths ?? [];

				if (indexedArgs) {
					const promises = indexedArgs.sourcePathIds.map(async (id) => {
						const path = await libraryClient.query(['files.getPath', id]);
						if (path) sources.push(path);
					});

					await Promise.all(promises);
				}

				let targetDir = path;

				// Prefix current path with the parent location path
				if (explorer.parent?.type === 'Location') {
					targetDir = explorer.parent.location.path + path;
				}

				await mutation.mutateAsync({
					sources: sources,
					target_dir: targetDir
				});

				explorerStore.cutCopyState = { type: 'Idle' };
				toast.success(t(`${type.toLowerCase()}_success`));
			}

			if (indexedArgs && explorer.parent?.type === 'Location') {
				const mutation = type === 'Copy' ? copyFiles : cutFiles;

				await mutation.mutateAsync({
					source_location_id: indexedArgs.sourceLocationId,
					sources_file_path_ids: indexedArgs.sourcePathIds,
					target_location_id: explorer.parent.location.id,
					target_location_relative_directory_path: path
				});

				explorerStore.cutCopyState = { type: 'Idle' };
				toast.success(t(`${type.toLowerCase()}_success`));
			}
		} catch (error) {
			toast.error({
				title: t(type === 'Copy' ? 'failed_to_copy_file' : 'failed_to_cut_file'),
				body: t('error_message', { error })
			});
		}
	}

	return { copy, cut, duplicate, paste };
};
