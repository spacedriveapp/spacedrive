import {
	Clipboard,
	FilePlus,
	FileX,
	FolderPlus,
	Hash,
	Image,
	Notepad,
	Repeat,
	Share,
	ShieldCheck
} from '@phosphor-icons/react';
import { PropsWithChildren } from 'react';
import { useLibraryMutation, useSelector } from '@sd/client';
import { ContextMenu as CM, ModifierKeys, toast } from '@sd/ui';
import { useLocale, useOperatingSystem } from '~/hooks';
import { useQuickRescan } from '~/hooks/useQuickRescan';
import { keybindForOs } from '~/util/keybinds';

import { useExplorerContext } from './Context';
import { CopyAsPathBase } from './CopyAsPath';
import { useExplorerCopyPaste } from './hooks/useExplorerCopyPaste';
import { RevealInNativeExplorerBase } from './RevealInNativeExplorer';
import { explorerStore } from './store';
import { useExplorerSearchParams } from './util';

export default (props: PropsWithChildren) => {
	const os = useOperatingSystem();
	const keybind = keybindForOs(os);
	const [{ path: currentPath }] = useExplorerSearchParams();
	const cutCopyState = useSelector(explorerStore, (s) => s.cutCopyState);
	const rescan = useQuickRescan();
	const { parent } = useExplorerContext();

	const generateThumbsForLocation = useLibraryMutation('jobs.generateThumbsForLocation');
	// const generateLabelsForLocation = useLibraryMutation('jobs.generateLabelsForLocation');
	const objectValidator = useLibraryMutation('jobs.objectValidator');
	const rescanLocation = useLibraryMutation('locations.subPathRescan');
	const createFolder = useLibraryMutation(['files.createFolder'], {
		onError: (e) => {
			toast.error({
				title: t('create_folder_error'),
				body: t('error_message', { error: e })
			});
			console.error(e);
		},
		onSuccess: (folder) => {
			toast.success({
				title: t('create_folder_success', {
					name: folder
				})
			});
			rescan();
		}
	});
	const createFile = useLibraryMutation(['files.createFile'], {
		onError: (e) => {
			toast.error({ title: t('create_file_error'), body: t('error_message', { error: e }) });
			console.error(e);
		},
		onSuccess: (file) => {
			toast.success({
				title: t('create_file_success', {
					name: file
				})
			});
			rescan();
		}
	});
	const createEphemeralFolder = useLibraryMutation(['ephemeralFiles.createFolder'], {
		onError: (e) => {
			toast.error({
				title: t('create_folder_error'),
				body: t('error_message', { error: e })
			});
			console.error(e);
		},
		onSuccess: (folder) => {
			toast.success({
				title: t('create_folder_success', {
					name: folder
				})
			});
			rescan();
		}
	});
	const createEphemeralFile = useLibraryMutation(['ephemeralFiles.createFile'], {
		onError: (e) => {
			toast.error({ title: t('create_file_error'), body: t('error_message', { error: e }) });
			console.error(e);
		},
		onSuccess: (file) => {
			toast.success({
				title: t('create_file_success', {
					name: file
				})
			});
			rescan();
		}
	});

	const { paste } = useExplorerCopyPaste();

	const { t } = useLocale();

	return (
		<CM.Root trigger={props.children}>
			{(parent?.type === 'Location' || parent?.type === 'Ephemeral') && (
				<>
					{cutCopyState.type !== 'Idle' && (
						<>
							<CM.Item
								label={t('paste')}
								keybind={keybind([ModifierKeys.Control], ['V'])}
								onClick={paste}
								icon={Clipboard}
							/>

							<CM.Item
								label={t('deselect')}
								onClick={() => {
									explorerStore.cutCopyState = {
										type: 'Idle'
									};
								}}
								icon={FileX}
							/>

							<CM.Separator />
						</>
					)}
					<CM.SubMenu label={t('new')}>
						<CM.Item
							label={t('new_folder')}
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
						<CM.Separator />
						<CM.Item
							label={t('text_file')}
							icon={Notepad}
							onClick={() => {
								if (parent?.type === 'Location') {
									createFile.mutate({
										location_id: parent.location.id,
										sub_path: currentPath || null,
										name: null,
										context: 'text'
									});
								} else if (parent?.type === 'Ephemeral') {
									createEphemeralFile.mutate({
										path: parent?.path,
										context: 'text',
										name: null
									});
								}
							}}
						/>
						<CM.Item
							label={t('empty_file')}
							icon={FilePlus}
							onClick={() => {
								if (parent?.type === 'Location') {
									createFile.mutate({
										location_id: parent.location.id,
										sub_path: currentPath || null,
										name: null,
										context: 'empty'
									});
								} else if (parent?.type === 'Ephemeral') {
									createEphemeralFile.mutate({
										path: parent?.path,
										context: 'empty',
										name: null
									});
								}
							}}
						/>
					</CM.SubMenu>
				</>
			)}

			<CM.Item
				label={t('share')}
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
					<CM.SubMenu label={t('more_actions')}>
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
										title: t('failed_to_reindex_location'),
										body: t('error_message', { error })
									});
								}
							}}
							label={t('reindex')}
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
										title: t('failed_to_generate_thumbnails'),
										body: t('error_message', { error })
									});
								}
							}}
							label={t('regen_thumbnails')}
							icon={Image}
						/>

						{/* <CM.Item
							onClick={async () => {
								try {
									await generateLabelsForLocation.mutateAsync({
										id: parent.location.id,
										path: currentPath ?? '/',
										regenerate: true
									});
								} catch (error) {
									toast.error({
										title: t('failed_to_generate_labels'),
										body: t('error_message', { error })
									});
								}
							}}
							label={t('regen_labels')}
							icon={Hash}
						/> */}

						<CM.Item
							onClick={async () => {
								try {
									objectValidator.mutateAsync({
										id: parent.location.id,
										path: currentPath ?? '/'
									});
								} catch (error) {
									toast.error({
										title: t('failed_to_generate_checksum'),
										body: t('error_message', { error })
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
