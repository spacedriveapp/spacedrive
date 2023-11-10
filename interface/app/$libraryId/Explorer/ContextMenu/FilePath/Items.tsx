import { Image, Package, Trash, TrashSimple } from '@phosphor-icons/react';
import { libraryClient, useLibraryMutation } from '@sd/client';
import {
	ContextMenu,
	dialogManager,
	keySymbols,
	ModifierKeys,
	modifierSymbols,
	toast
} from '@sd/ui';
import { Menu } from '~/components/Menu';
import { useOperatingSystem } from '~/hooks';
import { useKeybindFactory } from '~/hooks/useKeybindFactory';
import { useQuickRescan } from '~/hooks/useQuickRescan';
import { isNonEmpty } from '~/util';

import { useExplorerContext } from '../../Context';
import { CopyAsPathBase } from '../../CopyAsPath';
import DeleteDialog from '../../FilePath/DeleteDialog';
import EraseDialog from '../../FilePath/EraseDialog';
import { ConditionalItem } from '../ConditionalItem';
import { useContextMenuContext } from '../context';

export * from './CutCopyItems';

export const Delete = new ConditionalItem({
	useCondition: () => {
		const { selectedFilePaths, selectedEphemeralPaths } = useContextMenuContext();

		if (!isNonEmpty(selectedFilePaths) && !isNonEmpty(selectedEphemeralPaths)) return null;

		return { selectedFilePaths, selectedEphemeralPaths };
	},
	Component: ({ selectedFilePaths, selectedEphemeralPaths }) => {
		const rescan = useQuickRescan();

		const dirCount =
			selectedFilePaths.filter((p) => p.is_dir).length +
			selectedEphemeralPaths.filter((p) => p.is_dir).length;
		const fileCount =
			selectedFilePaths.filter((p) => !p.is_dir).length +
			selectedEphemeralPaths.filter((p) => !p.is_dir).length;

		const indexedArgs =
			isNonEmpty(selectedFilePaths) && selectedFilePaths[0].location_id
				? {
						locationId: selectedFilePaths[0].location_id,
						rescan,
						pathIds: selectedFilePaths.map((p) => p.id)
				  }
				: undefined;

		const ephemeralArgs = isNonEmpty(selectedEphemeralPaths)
			? {
					paths: selectedEphemeralPaths.map((p) => p.path)
			  }
			: undefined;

		return (
			<Menu.Item
				icon={Trash}
				label="Delete"
				variant="danger"
				onClick={() =>
					dialogManager.create((dp) => (
						<DeleteDialog
							{...dp}
							indexedArgs={indexedArgs}
							ephemeralArgs={ephemeralArgs}
							dirCount={dirCount}
							fileCount={fileCount}
						/>
					))
				}
			/>
		);
	}
});

export const CopyAsPath = new ConditionalItem({
	useCondition: () => {
		const { selectedFilePaths, selectedEphemeralPaths } = useContextMenuContext();
		if (
			!isNonEmpty(selectedFilePaths) ||
			selectedFilePaths.length > 1 ||
			!isNonEmpty(selectedEphemeralPaths) ||
			selectedEphemeralPaths.length > 1 ||
			(selectedFilePaths.length === 1 && selectedEphemeralPaths.length === 1) // should never happen
		)
			return null;

		return { selectedFilePaths, selectedEphemeralPaths };
	},
	Component: ({ selectedFilePaths, selectedEphemeralPaths }) => {
		if (selectedFilePaths.length === 1) {
			return (
				<CopyAsPathBase
					getPath={() => libraryClient.query(['files.getPath', selectedFilePaths[0].id])}
				/>
			);
		} else if (selectedEphemeralPaths.length === 1) {
			return <CopyAsPathBase getPath={async () => selectedEphemeralPaths[0].path} />;
		}
	}
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
			<Menu.Item
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
		<Menu.Item
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
							toast.error({
								title: `Failed to rescan location`,
								body: `Error: ${error}.`
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
								path: selectedFilePaths[0]?.materialized_path ?? '/',
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
			</>
		);
	}
});
