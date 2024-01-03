import {
	Clipboard,
	FileX,
	FolderPlus,
	Image,
	Repeat,
	Share,
	ShieldCheck
} from '@phosphor-icons/react';
import { PropsWithChildren } from 'react';
import { useLibraryMutation } from '@sd/client';
import { ContextMenu as CM, ModifierKeys, toast } from '@sd/ui';
import { useLocale, useOperatingSystem } from '~/hooks';
import { useQuickRescan } from '~/hooks/useQuickRescan';
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
	const rescan = useQuickRescan();
	const { parent } = useExplorerContext();

	const generateThumbsForLocation = useLibraryMutation('jobs.generateThumbsForLocation');
	const objectValidator = useLibraryMutation('jobs.objectValidator');
	const rescanLocation = useLibraryMutation('locations.subPathRescan');
	const copyFiles = useLibraryMutation('files.copyFiles');
	const copyEphemeralFiles = useLibraryMutation('ephemeralFiles.copyFiles');
	const cutFiles = useLibraryMutation('files.cutFiles');
	const cutEphemeralFiles = useLibraryMutation('ephemeralFiles.cutFiles');
	const createFolder = useLibraryMutation(['files.createFolder'], {
		onError: (e) => {
			toast.error({ title: 'Error creating folder', body: `Error: ${e}.` });
			console.error(e);
		},
		onSuccess: (folder) => {
			toast.success({ title: `Created new folder "${folder}"` });
			rescan();
		}
	});
	const createEphemeralFolder = useLibraryMutation(['ephemeralFiles.createFolder'], {
		onError: (e) => {
			toast.error({ title: 'Error creating folder', body: `Error: ${e}.` });
			console.error(e);
		},
		onSuccess: (folder) => {
			toast.success({ title: `Created new folder "${folder}"` });
			rescan();
		}
	});

	const { t } = useLocale();

	return (
		<CM.Root trigger={props.children}>
			{(parent?.type === 'Location' || parent?.type === 'Ephemeral') &&
			cutCopyState.type !== 'Idle' ? (
				<>
					<CM.Item
						label={t('paste')}
						keybind={keybind([ModifierKeys.Control], ['V'])}
						onClick={async () => {
							const path = currentPath ?? '/';
							const { type, sourceParentPath, indexedArgs, ephemeralArgs } =
								cutCopyState;

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

									if (
										parent?.type === 'Ephemeral' &&
										ephemeralArgs != undefined
									) {
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
											toast.error('File already exists in this location');
										}
										await cutFiles.mutateAsync({
											source_location_id: indexedArgs.sourceLocationId,
											sources_file_path_ids: [...indexedArgs.sourcePathIds],
											target_location_id: parent.location.id,
											target_location_relative_directory_path: path
										});
									}

									if (
										parent?.type === 'Ephemeral' &&
										ephemeralArgs != undefined
									) {
										if (sourceParentPath !== path) {
											await cutEphemeralFiles.mutateAsync({
												sources: [...ephemeralArgs.sourcePaths],
												target_dir: path
											});
										}
									}
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
						label={t('deselect')}
						onClick={() => {
							getExplorerStore().cutCopyState = {
								type: 'Idle'
							};
						}}
						icon={FileX}
					/>

					<CM.Separator />
				</>
			) : (
				<CM.Item
					label={t("new_folder")}
					icon={FolderPlus}
					onClick={() => {
						if (parent?.type === 'Location') {
							createFolder.mutate({
								location_id: parent.location.id,
								sub_path: currentPath || null,
								name: null
							});
						} else if (parent?.type === 'Ephemeral') {
							createEphemeralFolder.mutate({
								path: parent?.path,
								name: null
							});
						}
					}}
				/>
			)}

			<CM.Item
				label={t("share")}
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
					<CM.SubMenu label={t("more_actions")}>
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
							label={t('regen_thumbnails')}
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
							label={t('generate_checksums')}
							icon={ShieldCheck}
						/>
					</CM.SubMenu>
				</>
			)}
		</CM.Root>
	);
};
