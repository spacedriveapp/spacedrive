import { Clipboard, FileX, Image, Plus, Repeat, Share, ShieldCheck } from '@phosphor-icons/react';
import { PropsWithChildren } from 'react';
import { useLibraryMutation } from '@sd/client';
import { ContextMenu as CM, ModifierKeys, toast } from '@sd/ui';

import { useOperatingSystem } from '~/hooks';
import { keybindForOs } from '~/util/keybinds';
import { useExplorerContext } from './Context';
import { CopyAsPathBase } from './CopyAsPath';
import { RevealInNativeExplorerBase } from './RevealInNativeExplorer';
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
	const rescanLocation = useLibraryMutation('locations.subPathRescan');
	const copyFiles = useLibraryMutation('files.copyFiles');
	const cutFiles = useLibraryMutation('files.cutFiles');

	return (
		<CM.Root trigger={props.children}>
			{parent?.type === 'Location' && cutCopyState.type !== 'Idle' && (
				<>
					<CM.Item
						label="Paste"
						keybind={keybind([ModifierKeys.Control], ['V'])}
						onClick={async () => {
							const path = currentPath ?? '/';
							const { type, sourcePathIds, sourceParentPath, sourceLocationId } =
								cutCopyState;

							const sameLocation =
								sourceLocationId === parent.location.id &&
								sourceParentPath === path;

							try {
								if (type == 'Copy') {
									await copyFiles.mutateAsync({
										source_location_id: sourceLocationId,
										sources_file_path_ids: [...sourcePathIds],
										target_location_id: parent.location.id,
										target_location_relative_directory_path: path,
										target_file_name_suffix: sameLocation ? ' copy' : null
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
						}}
						icon={Clipboard}
					/>

					<CM.Item
						label="Deselect"
						onClick={() => {
							getExplorerStore().cutCopyState = {
								type: 'Idle'
							};
						}}
						icon={FileX}
					/>

					<CM.Separator />
				</>
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

			{parent?.type === 'Location' && (
				<>
					<RevealInNativeExplorerBase
						items={[{ Location: { id: parent.location.id } }]}
					/>
					<CM.SubMenu label="More actions..." icon={Plus}>
						<CopyAsPathBase path={`${parent.location.path}${currentPath ?? ''}`} />

						<CM.Item
							onClick={async () => {
								try {
									await rescanLocation.mutateAsync({
										location_id: parent.location.id,
										sub_path: currentPath ?? ''
									});
								} catch (error) {
									toast.error({
										title: `Failed to re-index location`,
										body: `Error: ${error}.`
									});
								}
							}}
							label="Re-index"
							icon={Repeat}
						/>

						<CM.Item
							onClick={async () => {
								try {
									await generateThumbsForLocation.mutateAsync({
										id: parent.location.id,
										path: currentPath ?? '/',
										regenerate: true
									});
								} catch (error) {
									toast.error({
										title: `Failed to generate thumbnails`,
										body: `Error: ${error}.`
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
									toast.error({
										title: `Failed to generate checksum`,
										body: `Error: ${error}.`
									});
								}
							}}
							label="Generate Checksums"
							icon={ShieldCheck}
						/>
					</CM.SubMenu>
				</>
			)}
		</CM.Root>
	);
};
