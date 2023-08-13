import { Image, Package, Trash, TrashSimple } from 'phosphor-react';
import { libraryClient, useLibraryContext, useLibraryMutation } from '@sd/client';
import { ContextMenu, ModifierKeys, dialogManager } from '@sd/ui';
import { showAlertDialog } from '~/components';
import { useKeybindFactory } from '~/hooks/useKeybindFactory';
import { isNonEmpty } from '~/util';
import { usePlatform } from '~/util/Platform';
import { useExplorerContext } from '../../Context';
import { CopyAsPathBase } from '../../CopyAsPath';
import DeleteDialog from '../../FilePath/DeleteDialog';
import EraseDialog from '../../FilePath/EraseDialog';
import { Conditional, ConditionalItem } from '../ConditionalItem';
import { useContextMenuContext } from '../context';
import OpenWith from './OpenWith';

export * from './CutCopyItems';

export const Delete = new ConditionalItem({
	useCondition: () => {
		const { selectedFilePaths } = useContextMenuContext();
		if (!isNonEmpty(selectedFilePaths)) return null;

		const locationId = selectedFilePaths[0].location_id;
		if (locationId === null) return null;

		return { selectedFilePaths, locationId };
	},
	Component: ({ selectedFilePaths, locationId }) => {
		const keybind = useKeybindFactory();

		return (
			<ContextMenu.Item
				icon={Trash}
				label="Delete"
				variant="danger"
				keybind={keybind([ModifierKeys.Control], ['Delete'])}
				onClick={() =>
					dialogManager.create((dp) => (
						<DeleteDialog
							{...dp}
							locationId={locationId}
							pathIds={selectedFilePaths.map((p) => p.id)}
						/>
					))
				}
			/>
		);
	}
});

export const CopyAsPath = new ConditionalItem({
	useCondition: () => {
		const { selectedFilePaths } = useContextMenuContext();
		if (!isNonEmpty(selectedFilePaths) || selectedFilePaths.length > 1) return null;

		return { selectedFilePaths };
	},
	Component: ({ selectedFilePaths }) => (
		<CopyAsPathBase
			getPath={() => libraryClient.query(['files.getPath', selectedFilePaths[0].id])}
		/>
	)
});

export const Compress = new ConditionalItem({
	useCondition: () => {
		const { selectedFilePaths } = useContextMenuContext();
		if (!isNonEmpty(selectedFilePaths)) return null;

		return { selectedFilePaths };
	},
	Component: ({ selectedFilePaths: _ }) => {
		const keybind = useKeybindFactory();

		return (
			<ContextMenu.Item
				label="Compress"
				icon={Package}
				keybind={keybind([ModifierKeys.Control], ['B'])}
				disabled
			/>
		);
	}
});

export const Crypto = new ConditionalItem({
	useCondition: () => {
		const { selectedFilePaths } = useContextMenuContext();
		if (!isNonEmpty(selectedFilePaths)) return null;

		return { selectedFilePaths };
	},
	Component: ({ selectedFilePaths: _ }) => {
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
	}
});

export const SecureDelete = new ConditionalItem({
	useCondition: () => {
		const { selectedFilePaths } = useContextMenuContext();
		if (!isNonEmpty(selectedFilePaths)) return null;

		const locationId = selectedFilePaths[0].location_id;
		if (locationId === null) return null;

		return { locationId, selectedFilePaths };
	},
	Component: ({ locationId, selectedFilePaths }) => (
		<ContextMenu.Item
			variant="danger"
			label="Secure delete"
			icon={TrashSimple}
			onClick={() =>
				dialogManager.create((dp) => (
					<EraseDialog {...dp} locationId={locationId} filePaths={selectedFilePaths} />
				))
			}
			disabled
		/>
	)
});

export const ParentFolderActions = new ConditionalItem({
	useCondition: () => {
		const { parent } = useExplorerContext();

		if (parent?.type !== 'Location') return null;

		return { parent };
	},
	Component: ({ parent }) => {
		const { selectedFilePaths } = useContextMenuContext();

		const fullRescan = useLibraryMutation('locations.fullRescan');
		const generateThumbnails = useLibraryMutation('jobs.generateThumbsForLocation');

		return (
			<>
				<ContextMenu.Item
					onClick={async () => {
						try {
							await fullRescan.mutateAsync({
								location_id: parent.location.id,
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
								id: parent.location.id,
								path: selectedFilePaths[0]?.materialized_path ?? '/'
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
	}
});

export const OpenOrDownload = new ConditionalItem({
	useCondition: () => {
		const { selectedFilePaths } = useContextMenuContext();
		const { openFilePaths } = usePlatform();

		if (!openFilePaths || !isNonEmpty(selectedFilePaths)) return null;

		return { openFilePaths, selectedFilePaths };
	},
	Component: ({ openFilePaths, selectedFilePaths }) => {
		const keybind = useKeybindFactory();
		const { platform } = usePlatform();
		const updateAccessTime = useLibraryMutation('files.updateAccessTime');

		const { library } = useLibraryContext();

		if (platform === 'web') return <ContextMenu.Item label="Download" />;
		else
			return (
				<>
					<ContextMenu.Item
						label="Open"
						keybind={keybind([ModifierKeys.Control], ['O'])}
						onClick={async () => {
							if (selectedFilePaths.length < 1) return;

							updateAccessTime
								.mutateAsync(
									selectedFilePaths.map((p) => p.object_id!).filter(Boolean)
								)
								.catch(console.error);

							try {
								await openFilePaths(
									library.uuid,
									selectedFilePaths.map((p) => p.id)
								);
							} catch (error) {
								showAlertDialog({
									title: 'Error',
									value: `Failed to open file, due to an error: ${error}`
								});
							}
						}}
					/>
					<Conditional items={[OpenWith]} />
				</>
			);
	}
});
