import { Copy, Scissors } from 'phosphor-react';
import { useLibraryMutation } from '@sd/client';
import { ContextMenu, ModifierKeys } from '@sd/ui';
import { showAlertDialog } from '~/components';
import { useKeybindFactory } from '~/hooks/useKeybindFactory';
import { isNonEmpty } from '~/util';
import { useExplorerContext } from '../../Context';
import { getExplorerStore } from '../../store';
import { useExplorerSearchParams } from '../../util';
import { ConditionalItem } from '../ConditionalItem';
import { useContextMenuContext } from '../context';

export const CutCopyItems = new ConditionalItem({
	useCondition: () => {
		const { parent } = useExplorerContext();
		const { selectedFilePaths } = useContextMenuContext();

		if (parent?.type !== 'Location' || !isNonEmpty(selectedFilePaths)) return null;

		return { locationId: parent.location.id, selectedFilePaths };
	},
	Component: ({ locationId, selectedFilePaths }) => {
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
							sourcePathIds: selectedFilePaths.map((p) => p.id),
							type: 'Cut'
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
							sourcePathIds: selectedFilePaths.map((p) => p.id),
							type: 'Copy'
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
								sources_file_path_ids: selectedFilePaths.map((p) => p.id),
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
	}
});
