import { Clipboard, FileX, Image, Plus, Repeat, Share, ShieldCheck } from 'phosphor-react';
import { PropsWithChildren, useMemo } from 'react';
import { useLibraryMutation } from '@sd/client';
import { ContextMenu as CM } from '@sd/ui';
import { useExplorerParams } from '~/app/$libraryId/location/$id';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import { usePlatform } from '~/util/Platform';

export const OpenInNativeExplorer = () => {
	const platform = usePlatform();
	const os = useOperatingSystem();

	const osFileBrowserName = useMemo(() => {
		if (os === 'macOS') {
			return 'Finder';
		} else {
			return 'Explorer';
		}
	}, [os]);

	return (
		<>
			{platform.openPath && (
				<CM.Item
					label={`Open in ${osFileBrowserName}`}
					keybind="⌘Y"
					onClick={() => {
						alert('TODO: Open in FS');
						// console.log('TODO', store.contextMenuActiveItem);
						// platform.openPath!('/Users/oscar/Desktop'); // TODO: Work out the file path from the backend
					}}
				/>
			)}
		</>
	);
};

export default (props: PropsWithChildren) => {
	const store = useExplorerStore();
	const params = useExplorerParams();

	const generateThumbsForLocation = useLibraryMutation('jobs.generateThumbsForLocation');
	const objectValidator = useLibraryMutation('jobs.objectValidator');
	const rescanLocation = useLibraryMutation('locations.fullRescan');
	const copyFiles = useLibraryMutation('files.copyFiles');
	const cutFiles = useLibraryMutation('files.cutFiles');

	return (
		<div className="relative">
			<CM.Root trigger={props.children}>
				<OpenInNativeExplorer />

				<CM.Separator />

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
				/>

				<CM.Separator />

				<CM.Item
					onClick={() => store.locationId && rescanLocation.mutate(store.locationId)}
					label="Re-index"
					icon={Repeat}
				/>

				<CM.Item
					label="Paste"
					keybind="⌘V"
					hidden={!store.cutCopyState.active}
					onClick={() => {
						if (store.cutCopyState.actionType == 'Copy') {
							store.locationId &&
								copyFiles.mutate({
									source_location_id: store.cutCopyState.sourceLocationId,
									source_path_id: store.cutCopyState.sourcePathId,
									target_location_id: store.locationId,
									target_path: params.path,
									target_file_name_suffix: null
								});
						} else {
							store.locationId &&
								cutFiles.mutate({
									source_location_id: store.cutCopyState.sourceLocationId,
									source_path_id: store.cutCopyState.sourcePathId,
									target_location_id: store.locationId,
									target_path: params.path
								});
						}
					}}
					icon={Clipboard}
				/>

				<CM.Item
					label="Deselect"
					hidden={!store.cutCopyState.active}
					onClick={() => {
						getExplorerStore().cutCopyState = {
							...store.cutCopyState,
							active: false
						};
					}}
					icon={FileX}
				/>

				<CM.SubMenu label="More actions..." icon={Plus}>
					<CM.Item
						onClick={() =>
							store.locationId &&
							generateThumbsForLocation.mutate({ id: store.locationId, path: '' })
						}
						label="Regen Thumbnails"
						icon={Image}
					/>
					<CM.Item
						onClick={() =>
							store.locationId && objectValidator.mutate({ id: store.locationId, path: '' })
						}
						label="Generate Checksums"
						icon={ShieldCheck}
					/>
				</CM.SubMenu>

				<CM.Separator />
			</CM.Root>
		</div>
	);
};
