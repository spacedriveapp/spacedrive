import { Copy, Scissors } from '@phosphor-icons/react';
import { useLibraryMutation } from '@sd/client';
import { ContextMenu, ModifierKeys, toast } from '@sd/ui';
import { useKeybindFactory } from '~/hooks/useKeybindFactory';
import { isNonEmpty } from '~/util';

import { useExplorerContext } from '../../Context';
import { explorerStore } from '../../store';
import { useExplorerSearchParams } from '../../util';
import { ConditionalItem } from '../ConditionalItem';
import { useContextMenuContext } from '../context';

export const CutCopyItems = new ConditionalItem({
	useCondition: () => {
		const { parent } = useExplorerContext();
		const { selectedFilePaths, selectedEphemeralPaths } = useContextMenuContext();

		if (
			(parent?.type !== 'Location' && parent?.type !== 'Ephemeral') ||
			(!isNonEmpty(selectedFilePaths) && !isNonEmpty(selectedEphemeralPaths))
		)
			return null;

		return { parent, selectedFilePaths, selectedEphemeralPaths };
	},
	Component: ({ parent, selectedFilePaths, selectedEphemeralPaths }) => {
		const keybind = useKeybindFactory();
		const [{ path }] = useExplorerSearchParams();

		const copyFiles = useLibraryMutation('files.copyFiles');
		const copyEphemeralFiles = useLibraryMutation('ephemeralFiles.copyFiles');

		const indexedArgs =
			parent.type === 'Location' && isNonEmpty(selectedFilePaths)
				? {
						sourceLocationId: parent.location.id,
						sourcePathIds: selectedFilePaths.map((p) => p.id)
				  }
				: undefined;

		const ephemeralArgs =
			parent.type === 'Ephemeral' && isNonEmpty(selectedEphemeralPaths)
				? { sourcePaths: selectedEphemeralPaths.map((p) => p.path) }
				: undefined;

		return (
			<>
				<ContextMenu.Item
					label="Cut"
					keybind={keybind([ModifierKeys.Control], ['X'])}
					onClick={() => {
						explorerStore.cutCopyState = {
							sourceParentPath: path ?? '/',
							indexedArgs,
							ephemeralArgs,
							type: 'Cut'
						};
					}}
					icon={Scissors}
				/>

				<ContextMenu.Item
					label="Copy"
					keybind={keybind([ModifierKeys.Control], ['C'])}
					onClick={() => {
						explorerStore.cutCopyState = {
							sourceParentPath: path ?? '/',
							indexedArgs,
							ephemeralArgs,
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
							if (parent.type === 'Location' && isNonEmpty(selectedFilePaths)) {
								await copyFiles.mutateAsync({
									source_location_id: parent.location.id,
									sources_file_path_ids: selectedFilePaths.map((p) => p.id),
									target_location_id: parent.location.id,
									target_location_relative_directory_path: path ?? '/'
								});
							}

							if (parent.type === 'Ephemeral' && isNonEmpty(selectedEphemeralPaths)) {
								await copyEphemeralFiles.mutateAsync({
									sources: selectedEphemeralPaths.map((p) => p.path),
									target_dir: path ?? '/'
								});
							}
						} catch (error) {
							toast.error({
								title: 'Failed to duplicate file',
								body: `Error: ${error}.`
							});
						}
					}}
				/>
			</>
		);
	}
});
