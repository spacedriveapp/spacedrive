import { Clipboard, FileX, Image, Plus, Repeat, Share, ShieldCheck } from 'phosphor-react';
import { PropsWithChildren } from 'react';
import { useLibraryMutation } from '@sd/client';
import { ContextMenu as CM, ModifierKeys } from '@sd/ui';
import { showAlertDialog } from '~/components';
import { useOperatingSystem } from '~/hooks';
import { keybindForOs } from '~/util/keybinds';
import { useExplorerContext } from './Context';
import { SharedItems } from './ContextMenu';
import { getExplorerStore, useExplorerStore } from './store';
import { useExplorerSearchParams } from './util';

export default (props: PropsWithChildren) => {
	const os = useOperatingSystem();
	const keybind = keybindForOs(os);
	const [{ path: currentPath }] = useExplorerSearchParams();
	const { cutCopyState } = useExplorerStore();

	const { parent } = useExplorerContext();

	const generateThumbsForLocation = useLibraryMutation('jobs.generateThumbsForLocation');
	const objectValidator = useLibraryMutation('jobs.objectValidator');
	const rescanLocation = useLibraryMutation('locations.fullRescan');
	const copyFiles = useLibraryMutation('files.copyFiles');
	const cutFiles = useLibraryMutation('files.cutFiles');

	return (
		<CM.Root trigger={props.children}>
			{parent?.type === 'Location' && (
				<SharedItems.RevealInNativeExplorer locationId={parent.location.id} />
			)}

			<CM.Item
				label="Share"
				icon={Share}
				onClick={(e) => {
					e.preventDefault();

					navigator.share?.({
						title: 'Spacedrive',
						text: 'Check out this cool app',
						url: 'https://spacedrive.com'
					});
				}}
				disabled
			/>

			<CM.Separator />

			{parent?.type === 'Location' && (
				<>
					<CM.Item
						onClick={async () => {
							try {
								await rescanLocation.mutateAsync({ location_id: parent.location.id, reidentify_objects: false });
							} catch (error) {
								showAlertDialog({
									title: 'Error',
									value: `Failed to re-index location, due to an error: ${error}`
								});
							}
						}}
						label="Re-index"
						icon={Repeat}
					/>

					<CM.Item
						label="Paste"
						keybind={keybind([ModifierKeys.Control], ['V'])}
						hidden={!cutCopyState.active}
						onClick={async () => {
							const path = currentPath ?? '/';
							const { actionType, sourcePathId, sourceParentPath, sourceLocationId } =
								cutCopyState;
							const sameLocation =
								sourceLocationId === parent.location.id &&
								sourceParentPath === path;
							try {
								if (actionType == 'Copy') {
									await copyFiles.mutateAsync({
										source_location_id: sourceLocationId,
										sources_file_path_ids: [sourcePathId],
										target_location_id: parent.location.id,
										target_location_relative_directory_path: path,
										target_file_name_suffix: sameLocation ? ' copy' : null
									});
								} else if (sameLocation) {
									showAlertDialog({
										title: 'Error',
										value: `File already exists in this location`
									});
								} else {
									await cutFiles.mutateAsync({
										source_location_id: sourceLocationId,
										sources_file_path_ids: [sourcePathId],
										target_location_id: parent.location.id,
										target_location_relative_directory_path: path
									});
								}
							} catch (error) {
								showAlertDialog({
									title: 'Error',
									value: `Failed to ${actionType.toLowerCase()} file, due to an error: ${error}`
								});
							}
						}}
						icon={Clipboard}
					/>
				</>
			)}

			<CM.Item
				label="Deselect"
				hidden={!cutCopyState.active}
				onClick={() => {
					getExplorerStore().cutCopyState = {
						...cutCopyState,
						active: false
					};
				}}
				icon={FileX}
			/>

			{parent?.type === 'Location' && (
				<CM.SubMenu label="More actions..." icon={Plus}>
					<CM.Item
						onClick={async () => {
							try {
								await generateThumbsForLocation.mutateAsync({
									id: parent.location.id,
									path: currentPath ?? '/'
								});
							} catch (error) {
								showAlertDialog({
									title: 'Error',
									value: `Failed to generate thumbanails, due to an error: ${error}`
								});
							}
						}}
						label="Regen Thumbnails"
						icon={Image}
					/>

					<CM.Item
						onClick={async () => {
							try {
								objectValidator.mutateAsync({
									id: parent.location.id,
									path: currentPath ?? '/'
								});
							} catch (error) {
								showAlertDialog({
									title: 'Error',
									value: `Failed to generate checksum, due to an error: ${error}`
								});
							}
						}}
						label="Generate Checksums"
						icon={ShieldCheck}
					/>
				</CM.SubMenu>
			)}
		</CM.Root>
	);
};
