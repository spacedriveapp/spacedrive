import { FileX, Share as ShareIcon } from 'phosphor-react';
import { useMemo } from 'react';
import { ExplorerItem, FilePath, useLibraryContext } from '@sd/client';
import { ContextMenu, ModifierKeys } from '@sd/ui';
import { useOperatingSystem } from '~/hooks';
import { useKeybindFactory } from '~/hooks/useKeybindFactory';
import { usePlatform } from '~/util/Platform';
import { useExplorerViewContext } from '../ViewContext';
import { getExplorerStore, useExplorerStore } from '../store';

export const OpenQuickView = ({ item }: { item: ExplorerItem }) => {
	const keybind = useKeybindFactory();

	return (
		<ContextMenu.Item
			label="Quick view"
			keybind={keybind([], [' '])}
			onClick={() => (getExplorerStore().quickViewObject = item)}
		/>
	);
};

export const Details = () => {
	const { showInspector } = useExplorerStore();
	const keybind = useKeybindFactory();

	return (
		<>
			{!showInspector && (
				<ContextMenu.Item
					label="Details"
					keybind={keybind([ModifierKeys.Control], ['I'])}
					// icon={Sidebar}
					onClick={() => (getExplorerStore().showInspector = true)}
				/>
			)}
		</>
	);
};

export const Rename = () => {
	const explorerStore = useExplorerStore();
	const keybind = useKeybindFactory();
	const explorerView = useExplorerViewContext();

	return (
		<>
			{explorerStore.layoutMode !== 'media' && (
				<ContextMenu.Item
					label="Rename"
					keybind={keybind([], ['Enter'])}
					onClick={() => explorerView.setIsRenaming(true)}
				/>
			)}
		</>
	);
};

export const OpenInNativeExplorer = (props: { locationId: number } | { filePath: FilePath }) => {
	const os = useOperatingSystem();
	const keybind = useKeybindFactory();
	const platform = usePlatform();
	const { library } = useLibraryContext();

	const osFileBrowserName = useMemo(() => {
		const lookup: Record<string, string> = {
			macOS: 'Finder',
			windows: 'Explorer'
		};

		return lookup[os] ?? 'file manager';
	}, [os]);

	return (
		<>
			{/* {'filePath' in props && platform.openFilePaths && ( */}
			{/* 	<ContextMenu.Item */}
			{/* 		label={`Open in ${osFileBrowserName}`} */}
			{/* 		keybind={keybind([ModifierKeys.Control], ['Y'])} */}
			{/* 		onClick={() => platform.openFilePaths(library.uuid, [])} */}
			{/* 	/> */}
			{/* )} */}
			{/* {'locationId' in props && platform.openLocations && ( */}
			{/* 	<ContextMenu.Item */}
			{/* 		label={`Open in ${osFileBrowserName}`} */}
			{/* 		keybind={keybind([ModifierKeys.Control], ['Y'])} */}
			{/* 		onClick={() => platform.openLocations(library.uuid, [])} */}
			{/* 	/> */}
			{/* )} */}
		</>
	);
};

export const Deselect = () => {
	const { cutCopyState } = useExplorerStore();

	return (
		<ContextMenu.Item
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
	);
};

export const Share = () => {
	return (
		<>
			<ContextMenu.Item
				label="Share"
				icon={ShareIcon}
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
		</>
	);
};
