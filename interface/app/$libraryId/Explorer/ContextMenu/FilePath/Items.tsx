import { Image, Package, Trash, TrashSimple } from 'phosphor-react';
import { FilePath, useLibraryContext, useLibraryMutation } from '@sd/client';
import { ContextMenu, ModifierKeys, dialogManager } from '@sd/ui';
import { showAlertDialog } from '~/components';
import { useKeybindFactory } from '~/hooks/useKeybindFactory';
import { usePlatform } from '~/util/Platform';
import DeleteDialog from '../../FilePath/DeleteDialog';
import EraseDialog from '../../FilePath/EraseDialog';
import OpenWith from './OpenWith';

export * from './CutCopyItems';

interface FilePathProps {
	filePath: FilePath;
}

export const Delete = ({ filePath }: FilePathProps) => {
	const keybind = useKeybindFactory();

	const locationId = filePath.location_id;

	return (
		<>
			{locationId != null && (
				<ContextMenu.Item
					icon={Trash}
					label="Delete"
					variant="danger"
					keybind={keybind([ModifierKeys.Control], ['Delete'])}
					onClick={() =>
						dialogManager.create((dp) => (
							<DeleteDialog {...dp} location_id={locationId} path_id={filePath.id} />
						))
					}
				/>
			)}
		</>
	);
};

export const Compress = (_: FilePathProps) => {
	const keybind = useKeybindFactory();

	return (
		<ContextMenu.Item
			label="Compress"
			icon={Package}
			keybind={keybind([ModifierKeys.Control], ['B'])}
			disabled
		/>
	);
};

export const Crypto = (_: FilePathProps) => {
	return (
		<>
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
		</>
	);
};

export const SecureDelete = ({ filePath }: FilePathProps) => {
	const locationId = filePath.location_id;

	return (
		<>
			{locationId && (
				<ContextMenu.Item
					variant="danger"
					label="Secure delete"
					icon={TrashSimple}
					onClick={() =>
						dialogManager.create((dp) => (
							<EraseDialog {...dp} location_id={locationId} path_id={filePath.id} />
						))
					}
					disabled
				/>
			)}
		</>
	);
};

export const ParentFolderActions = ({
	filePath,
	locationId
}: FilePathProps & { locationId: number }) => {
	const fullRescan = useLibraryMutation('locations.fullRescan');
	const generateThumbnails = useLibraryMutation('jobs.generateThumbsForLocation');

	return (
		<>
			<ContextMenu.Item
				onClick={async () => {
					try {
						await fullRescan.mutateAsync({
							location_id: locationId,
							reidentify_objects: false
						});
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
							path: filePath.materialized_path ?? '/'
						});
					} catch (error) {
						showAlertDialog({
							title: 'Error',
							value: `Failed to generate thumbnails, due to an error: ${error}`
						});
					}
				}}
				label="Regen Thumbnails"
				icon={Image}
			/>
		</>
	);
};

export const OpenOrDownload = ({ filePath }: { filePath: FilePath }) => {
	const keybind = useKeybindFactory();
	const { platform, openFilePaths: openFilePath } = usePlatform();
	const updateAccessTime = useLibraryMutation('files.updateAccessTime');

	const { library } = useLibraryContext();

	if (platform === 'web') return <ContextMenu.Item label="Download" />;
	else
		return (
			<>
				{openFilePath && (
					<ContextMenu.Item
						label="Open"
						keybind={keybind([ModifierKeys.Control], ['O'])}
						onClick={async () => {
							if (filePath.object_id)
								updateAccessTime
									.mutateAsync(filePath.object_id)
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
		);
};
