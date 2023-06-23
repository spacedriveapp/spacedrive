import {
	ArrowBendUpRight,
	Copy,
	FileX,
	Image,
	Package,
	Plus,
	Scissors,
	Share,
	TagSimple,
	Trash,
	TrashSimple
} from 'phosphor-react';
import { useLocation } from 'react-router-dom';
import {
	ExplorerItem,
	getItemFilePath,
	getItemObject,
	useLibraryContext,
	useLibraryMutation
} from '@sd/client';
import { ContextMenu, ModifierKeys, dialogManager } from '@sd/ui';
import { showAlertDialog } from '~/components';
import {
	getExplorerStore,
	useExplorerStore,
	useOperatingSystem,
	useZodSearchParams
} from '~/hooks';
import { usePlatform } from '~/util/Platform';
import { keybindForOs } from '~/util/keybinds';
import AssignTagMenuItems from '../AssignTagMenuItems';
import { OpenInNativeExplorer } from '../ContextMenu';
import { useExplorerViewContext } from '../ViewContext';
import OpenWith from './ContextMenu/OpenWith';
// import DecryptDialog from './DecryptDialog';
import DeleteDialog from './DeleteDialog';
// import EncryptDialog from './EncryptDialog';
import EraseDialog from './EraseDialog';

interface Props {
	data?: ExplorerItem;
}

export default ({ data }: Props) => {
	const os = useOperatingSystem();
	const keybind = keybindForOs(os);
	const location = useLocation();
	const objectData = data ? getItemObject(data) : null;
	const explorerView = useExplorerViewContext();
	const explorerStore = useExplorerStore();
	const [{ path: currentPath }] = useZodSearchParams();
	const { cutCopyState, showInspector, ...store } = useExplorerStore();

	const isLocation =
		location.pathname.includes('/location/') && explorerStore.layoutMode !== 'media';

	// const keyManagerUnlocked = useLibraryQuery(['keys.isUnlocked']).data ?? false;
	// const mountedKeys = useLibraryQuery(['keys.listMounted']);
	// const hasMountedKeys = mountedKeys.data?.length ?? 0 > 0;

	const copyFiles = useLibraryMutation('files.copyFiles');
	const fullRescan = useLibraryMutation('locations.fullRescan');
	const removeFromRecents = useLibraryMutation('files.removeAccessTime');
	const generateThumbnails = useLibraryMutation('jobs.generateThumbsForLocation');

	if (!data) return null;

	const objectId = data.type == 'Path' ? data.item.object_id : null;
	const locationId = store.locationId ?? getItemFilePath(data)?.location_id;
	const objectDateAccessed =
		data.type == 'Path' ? data.item.object && data.item.object.date_accessed : null;

	return (
		<>
			<OpenOrDownloadOptions data={data} />

			<ContextMenu.Separator />

			{!showInspector && (
				<>
					<ContextMenu.Item
						label="Details"
						keybind={keybind([ModifierKeys.Control], ['I'])}
						// icon={Sidebar}
						onClick={() => (getExplorerStore().showInspector = true)}
					/>
					<ContextMenu.Separator />
				</>
			)}

			<OpenInNativeExplorer />

			<ContextMenu.Item
				hidden={explorerStore.layoutMode === 'media'}
				label="Rename"
				keybind={keybind([], ['Enter'])}
				onClick={() => explorerView.setIsRenaming(true)}
			/>

			{objectId && objectDateAccessed && (
				<ContextMenu.Item
					label="Remove from recents"
					onClick={async () => {
						try {
							await removeFromRecents.mutateAsync([objectId]);
						} catch (error) {
							showAlertDialog({
								title: 'Error',
								value: `Failed to remove file from recents, due to an error: ${error}`
							});
						}
					}}
				/>
			)}

			{locationId && (
				<>
					<ContextMenu.Item
						hidden={!isLocation}
						label="Cut"
						keybind={keybind([ModifierKeys.Control], ['X'])}
						onClick={() => {
							getExplorerStore().cutCopyState = {
								sourceParentPath: currentPath ?? '/',
								sourceLocationId: locationId,
								sourcePathId: data.item.id,
								actionType: 'Cut',
								active: true
							};
						}}
						icon={Scissors}
					/>

					<ContextMenu.Item
						hidden={!isLocation}
						label="Copy"
						keybind={keybind([ModifierKeys.Control], ['C'])}
						onClick={() => {
							getExplorerStore().cutCopyState = {
								sourceParentPath: currentPath ?? '/',
								sourceLocationId: locationId,
								sourcePathId: data.item.id,
								actionType: 'Copy',
								active: true
							};
						}}
						icon={Copy}
					/>

					<ContextMenu.Item
						hidden={!isLocation}
						label="Duplicate"
						keybind={keybind([ModifierKeys.Control], ['D'])}
						onClick={async () => {
							try {
								await copyFiles.mutateAsync({
									source_location_id: locationId,
									sources_file_path_ids: [data.item.id],
									target_location_id: locationId,
									target_location_relative_directory_path: currentPath ?? '/',
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
			)}

			<ContextMenu.Item
				label="Deselect"
				hidden={!(cutCopyState.active && isLocation)}
				onClick={() => {
					getExplorerStore().cutCopyState = {
						...cutCopyState,
						active: false
					};
				}}
				icon={FileX}
			/>

			<ContextMenu.Separator />

			<ContextMenu.Item
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

			<ContextMenu.Separator />

			{objectData && (
				<ContextMenu.SubMenu label="Assign tag" icon={TagSimple}>
					<AssignTagMenuItems objectId={objectData.id} />
				</ContextMenu.SubMenu>
			)}

			<ContextMenu.SubMenu label="More actions..." icon={Plus}>
				{/* <ContextMenu.Item
					label="Encrypt"
					icon={LockSimple}
					keybind="⌘E"
					onClick={() => {
						if (keyManagerUnlocked && hasMountedKeys) {
							dialogManager.create((dp) => (
								<EncryptDialog
									{...dp}
									location_id={store.locationId!}
									path_id={data.item.id}
								/>
							));
						} else if (!keyManagerUnlocked) {
							showAlertDialog({
								title: 'Key manager locked',
								value: 'The key manager is currently locked. Please unlock it and try again.'
							});
						} else if (!hasMountedKeys) {
							showAlertDialog({
								title: 'No mounted keys',
								value: 'No mounted keys were found. Please mount a key and try again.'
							});
						}
					}}
				/> */}
				{/* should only be shown if the file is a valid spacedrive-encrypted file (preferably going from the magic bytes) */}
				{/* <ContextMenu.Item
					label="Decrypt"
					icon={LockSimpleOpen}
					keybind="⌘D"
					onClick={() => {
						if (keyManagerUnlocked) {
							dialogManager.create((dp) => (
								<DecryptDialog
									{...dp}
									location_id={store.locationId!}
									path_id={data.item.id}
								/>
							));
						} else {
							showAlertDialog({
								title: 'Key manager locked',
								value: 'The key manager is currently locked. Please unlock it and try again.'
							});
						}
					}}
				/> */}
				<ContextMenu.Item
					label="Compress"
					icon={Package}
					keybind={keybind([ModifierKeys.Control], ['B'])}
					disabled
				/>
				<ContextMenu.SubMenu label="Convert to" icon={ArrowBendUpRight}>
					<ContextMenu.Item label="PNG" disabled />
					<ContextMenu.Item label="WebP" disabled />
				</ContextMenu.SubMenu>

				{locationId != null && (
					<>
						<ContextMenu.Item
							hidden={!isLocation}
							onClick={async () => {
								try {
									await fullRescan.mutateAsync(locationId);
								} catch (error) {
									showAlertDialog({
										title: 'Error',
										value: `Failed to rescan location, due to an error: ${error}`
									});
								}
							}}
							label="Rescan Directory"
							icon={Package}
						/>
						<ContextMenu.Item
							onClick={async () => {
								try {
									await generateThumbnails.mutateAsync({
										id: locationId,
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
						<ContextMenu.Item
							variant="danger"
							label="Secure delete"
							icon={TrashSimple}
							onClick={() =>
								dialogManager.create((dp) => (
									<EraseDialog
										{...dp}
										location_id={locationId}
										path_id={data.item.id}
									/>
								))
							}
							disabled
						/>
					</>
				)}
			</ContextMenu.SubMenu>

			<ContextMenu.Separator />

			{locationId != null && (
				<ContextMenu.Item
					icon={Trash}
					label="Delete"
					variant="danger"
					keybind={keybind([ModifierKeys.Control], ['Delete'])}
					onClick={() =>
						dialogManager.create((dp) => (
							<DeleteDialog {...dp} location_id={locationId} path_id={data.item.id} />
						))
					}
				/>
			)}
		</>
	);
};

const OpenOrDownloadOptions = (props: { data: ExplorerItem }) => {
	const os = useOperatingSystem();
	const keybind = keybindForOs(os);
	const { openFilePath } = usePlatform();
	const updateAccessTime = useLibraryMutation('files.updateAccessTime');
	const filePath = getItemFilePath(props.data);

	const { library } = useLibraryContext();

	if (os === 'browser') return <ContextMenu.Item label="Download" />;
	else
		return (
			<>
				{filePath && (
					<>
						{openFilePath && (
							<ContextMenu.Item
								label="Open"
								keybind={keybind([ModifierKeys.Control], ['O'])}
								onClick={async () => {
									if (props.data.type === 'Path' && props.data.item.object_id)
										updateAccessTime
											.mutateAsync(props.data.item.object_id)
											.catch(console.error);

									try {
										await openFilePath(library.uuid, [filePath.id]);
									} catch (error) {
										showAlertDialog({
											title: 'Error',
											value: `Failed to open file, due to an error: ${error}`
										});
									}
								}}
							/>
						)}
						<OpenWith filePath={filePath} />
					</>
				)}
				<ContextMenu.Item
					label="Quick view"
					keybind={keybind([], [' '])}
					onClick={() => (getExplorerStore().quickViewObject = props.data)}
				/>
			</>
		);
};
