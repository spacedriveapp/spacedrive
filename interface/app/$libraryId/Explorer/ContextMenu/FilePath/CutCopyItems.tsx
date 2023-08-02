import { Copy, Scissors } from 'phosphor-react';
import { FilePath, useLibraryMutation } from '@sd/client';
import { ContextMenu, ModifierKeys } from '@sd/ui';
import { showAlertDialog } from '~/components';
import { useKeybindFactory } from '~/hooks/useKeybindFactory';
import { getExplorerStore } from '../../store';
import { uniqueId, useExplorerSearchParams } from '../../util';

interface Props {
	locationId: number;
	filePath: FilePath;
}

export const CutCopyItems = ({ locationId, filePath }: Props) => {
	const keybind = useKeybindFactory();
	const [{ path }] = useExplorerSearchParams();

	const copyFiles = useLibraryMutation('files.copyFiles');

	return (
		<>
			<ContextMenu.Item
				label="Cut"
				keybind={keybind([ModifierKeys.Control], ['X'])}
				onClick={() => {
					getExplorerStore().cutCopyState = {
						sourceParentPath: path ?? '/',
						sourceLocationId: locationId,
						sourcePathId: filePath.id,
						actionType: 'Cut',
						active: true
					};
				}}
				icon={Scissors}
			/>

			<ContextMenu.Item
				label="Copy"
				keybind={keybind([ModifierKeys.Control], ['C'])}
				onClick={() => {
					getExplorerStore().cutCopyState = {
						sourceParentPath: path ?? '/',
						sourceLocationId: locationId,
						sourcePathId: filePath.id,
						actionType: 'Copy',
						active: true
					};
				}}
				icon={Copy}
			/>

			<ContextMenu.Item
				label="Duplicate"
				keybind={keybind([ModifierKeys.Control], ['D'])}
				onClick={async () => {
					try {
						await copyFiles.mutateAsync({
							source_location_id: locationId,
							sources_file_path_ids: [filePath.id],
							target_location_id: locationId,
							target_location_relative_directory_path: path ?? '/',
							target_file_name_suffix: ' copy'
						});
					} catch (error) {
						showAlertDialog({
							title: 'Error',
							value: `Failed to duplcate file, due to an error: ${error}`
						});
					}
				}}
			/>
		</>
	);
};
