import {
	ArrowBendUpRight,
	Copy,
	FileX,
	Package,
	Plus,
	Scissors,
	Share,
	TagSimple,
	Trash,
	TrashSimple
} from 'phosphor-react';
import { ExplorerItem, isObject, useLibraryContext, useLibraryMutation } from '@sd/client';
import { ContextMenu, dialogManager } from '@sd/ui';
import { getExplorerStore, useExplorerStore, useOperatingSystem } from '~/hooks';
import { usePlatform } from '~/util/Platform';
import AssignTagMenuItems from '../AssignTagMenuItems';
import { OpenInNativeExplorer } from '../ContextMenu';
import { useExplorerViewContext } from '../ViewContext';
import { getItemFilePath, useExplorerSearchParams } from '../util';
import OpenWith from './ContextMenu/OpenWith';
// import DecryptDialog from './DecryptDialog';
import DeleteDialog from './DeleteDialog';
// import EncryptDialog from './EncryptDialog';
import EraseDialog from './EraseDialog';

interface Props {
	data?: ExplorerItem;
}

export default ({ data }: Props) => {
	const store = useExplorerStore();
	const explorerView = useExplorerViewContext();
	const explorerStore = useExplorerStore();
	const [params] = useExplorerSearchParams();
	const objectData = data ? (isObject(data) ? data.item : data.item.object) : null;

	// const keyManagerUnlocked = useLibraryQuery(['keys.isUnlocked']).data ?? false;
	// const mountedKeys = useLibraryQuery(['keys.listMounted']);
	// const hasMountedKeys = mountedKeys.data?.length ?? 0 > 0;

	const copyFiles = useLibraryMutation('files.copyFiles');

	const removeFromRecents = useLibraryMutation('files.removeAccessTime');
	const generateThumbnails = useLibraryMutation('jobs.generateThumbsForLocation');
	const fullRescan = useLibraryMutation('locations.fullRescan');

	if (!data) return null;
	return (
		<>
			<OpenOrDownloadOptions data={data} />

			<ContextMenu.Separator />

			{!store.showInspector && (
				<>
					<ContextMenu.Item
						label="Details"
						keybind="⌘I"
						// icon={Sidebar}
						onClick={() => (getExplorerStore().showInspector = true)}
					/>
					<ContextMenu.Separator />
				</>
			)}

			<OpenInNativeExplorer />

			{explorerStore.layoutMode === 'media' || (
				<ContextMenu.Item
					label="Rename"
					keybind="Enter"
					onClick={() => explorerView.setIsRenaming(true)}
				/>
			)}

			{data.type == 'Path' && data.item.object && data.item.object.date_accessed && (
				<ContextMenu.Item
					label="Remove from recents"
					onClick={() =>
						data.item.object_id && removeFromRecents.mutate([data.item.object_id])
					}
				/>
			)}

			<ContextMenu.Item
				label="Cut"
				keybind="⌘X"
				onClick={() => {
					if (params.path === undefined) return;

					getExplorerStore().cutCopyState = {
						sourcePath: params.path,
						sourceLocationId: store.locationId!,
						sourcePathId: data.item.id,
						actionType: 'Cut',
						active: true
					};
				}}
				icon={Scissors}
			/>

			<ContextMenu.Item
				label="Copy"
				keybind="⌘C"
				onClick={() => {
					if (params.path === undefined) return;

					getExplorerStore().cutCopyState = {
						sourcePath: params.path,
						sourceLocationId: store.locationId!,
						sourcePathId: data.item.id,
						actionType: 'Copy',
						active: true
					};
				}}
				icon={Copy}
			/>

			<ContextMenu.Item
				label="Duplicate"
				keybind="⌘D"
				onClick={() => {
					if (params.path === undefined) return;

					copyFiles.mutate({
						source_location_id: store.locationId!,
						sources_file_path_ids: [data.item.id],
						target_location_id: store.locationId!,
						target_location_relative_directory_path: params.path,
						target_file_name_suffix: ' copy'
					});
				}}
			/>

			<ContextMenu.Item
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
				<ContextMenu.Item label="Compress" icon={Package} keybind="⌘B" />
				<ContextMenu.SubMenu label="Convert to" icon={ArrowBendUpRight}>
					<ContextMenu.Item label="PNG" />
					<ContextMenu.Item label="WebP" />
				</ContextMenu.SubMenu>
				<ContextMenu.Item
					onClick={() => {
						fullRescan.mutate(getExplorerStore().locationId!);
					}}
					label="Rescan Directory"
					icon={Package}
				/>
				<ContextMenu.Item
					onClick={() => {
						generateThumbnails.mutate({
							id: getExplorerStore().locationId!,
							path: '/'
						});
					}}
					label="Regen Thumbnails"
					icon={Package}
				/>
				<ContextMenu.Item
					variant="danger"
					label="Secure delete"
					icon={TrashSimple}
					onClick={() => {
						dialogManager.create((dp) => (
							<EraseDialog
								{...dp}
								location_id={getExplorerStore().locationId!}
								path_id={data.item.id}
							/>
						));
					}}
				/>
			</ContextMenu.SubMenu>

			<ContextMenu.Separator />

			<ContextMenu.Item
				icon={Trash}
				label="Delete"
				variant="danger"
				keybind="⌘DEL"
				onClick={() => {
					dialogManager.create((dp) => (
						<DeleteDialog
							{...dp}
							location_id={getExplorerStore().locationId!}
							path_id={data.item.id}
						/>
					));
				}}
			/>
		</>
	);
};

const OpenOrDownloadOptions = (props: { data: ExplorerItem }) => {
	const os = useOperatingSystem();
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
								keybind="⌘O"
								onClick={() => {
									props.data.type === 'Path' &&
										props.data.item.object_id &&
										updateAccessTime.mutate(props.data.item.object_id);

									// FIXME: treat error properly
									openFilePath(library.uuid, [filePath.id]);
								}}
							/>
						)}
						<OpenWith filePath={filePath} />
					</>
				)}
				<ContextMenu.Item
					label="Quick view"
					keybind="␣"
					onClick={() => (getExplorerStore().quickViewObject = props.data)}
				/>
			</>
		);
};
