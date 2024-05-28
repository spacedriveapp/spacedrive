import {
	useBridgeQuery,
	useItemsAsEphemeralPaths,
	useItemsAsFilePaths,
	useLibraryMutation,
	type ExplorerItem
} from '@sd/client';
import { dialogManager } from '@sd/ui';
import DeleteDialog from '~/app/$libraryId/Explorer/FilePath/DeleteDialog';
import { isNonEmpty } from '~/util';

import { useShortcut } from './useShortcut';

export const useKeyDeleteFile = (selectedItems: Set<ExplorerItem>, locationId?: number | null) => {
	const filePaths = useItemsAsFilePaths([...selectedItems]);
	const ephemeralPaths = useItemsAsEphemeralPaths([...selectedItems]);
	const node = useBridgeQuery(['nodeState']);
	const deleteFile = useLibraryMutation('files.deleteFiles');
	const deleteEphemeralFile = useLibraryMutation('ephemeralFiles.deleteFiles');
	const moveToTrashFile = useLibraryMutation('files.moveToTrash');
	const moveToTrashEphemeralFile = useLibraryMutation('ephemeralFiles.moveToTrash');

	const deleteHandler = (e: KeyboardEvent) => {
		e.preventDefault();

		if ((locationId == null || !isNonEmpty(filePaths)) && !isNonEmpty(ephemeralPaths)) return;

		const indexedArgs =
			locationId != null && isNonEmpty(filePaths)
				? { locationId, pathIds: filePaths.map((p) => p.id) }
				: undefined;
		const ephemeralArgs = isNonEmpty(ephemeralPaths)
			? { paths: ephemeralPaths.map((p) => p.path) }
			: undefined;

		let dirCount = 0;
		let fileCount = 0;

		for (const entry of [...filePaths, ...ephemeralPaths]) {
			dirCount += entry.is_dir ? 1 : 0;
			fileCount += entry.is_dir ? 0 : 1;
		}

		if (node.data?.delete_prompt.option === 'ShowPrompt') {
			dialogManager.create((dp) => (
				<DeleteDialog
					{...dp}
					indexedArgs={indexedArgs}
					ephemeralArgs={ephemeralArgs}
					dirCount={dirCount}
					fileCount={fileCount}
				/>
			));
		} else if (node.data?.delete_prompt.option === 'SendTrash') {
			if (locationId != null && isNonEmpty(filePaths)) {
				moveToTrashFile.mutate({ location_id: locationId, file_path_ids: filePaths.map((p) => p.id) });
			} else if (isNonEmpty(ephemeralPaths)) {
				const { paths } = ephemeralArgs!;
				moveToTrashEphemeralFile.mutate(paths);
			}
		} else {
			if (locationId != null && isNonEmpty(filePaths)) {
				deleteFile.mutate({ location_id: locationId, file_path_ids: filePaths.map((p) => p.id) });
			} else if (isNonEmpty(ephemeralPaths)) {
				const { paths } = ephemeralArgs!;
				deleteEphemeralFile.mutate(paths);
			}
		}
	};

	useShortcut('delItem', deleteHandler);
};
