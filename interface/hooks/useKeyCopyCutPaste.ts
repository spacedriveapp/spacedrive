import { getExplorerStore, useExplorerStore } from "~/app/$libraryId/Explorer/store";
import { useExplorerSearchParams } from "~/app/$libraryId/Explorer/util";
import { useExplorerContext } from "~/app/$libraryId/Explorer/Context";
import { useKeyMatcher } from "./useKeyMatcher";
import { useKeys } from "rooks";
import {useItemsAsFilePaths, useLibraryMutation} from '@sd/client';
import {toast} from '@sd/ui';


export const useKeyCopyCutPaste = () => {
	const {cutCopyState} = useExplorerStore();
	const [{ path }] = useExplorerSearchParams();

	const metaCtrlKey = useKeyMatcher('Meta').key;
	const copyFiles = useLibraryMutation('files.copyFiles');
	const cutFiles = useLibraryMutation('files.cutFiles');
	const explorer = useExplorerContext();
	const selectedFilePaths = useItemsAsFilePaths(Array.from(explorer.selectedItems));

	useKeys([metaCtrlKey, 'KeyC'], (e) => {
		e.stopPropagation();
;		if (explorer.parent?.type === 'Location') {
			getExplorerStore().cutCopyState = {
				sourceParentPath: path ?? '/',
				sourceLocationId: explorer.parent.location.id,
				sourcePathIds: selectedFilePaths.map((p) => p.id),
				type: 'Copy'
			};
		}
	});

	useKeys([metaCtrlKey, 'KeyX'], (e) => {
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

	useKeys([metaCtrlKey, 'KeyV'], async (e) => {
		e.stopPropagation();
		const parent = explorer.parent;
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
}
