import { FileX, Share as ShareIcon } from '@phosphor-icons/react';
import { useMemo } from 'react';
import { useSelector } from '@sd/client';
import { ContextMenu, ModifierKeys } from '@sd/ui';
import { Menu } from '~/components/Menu';
import { useOperatingSystem } from '~/hooks';
import { useKeybindFactory } from '~/hooks/useKeybindFactory';
import { isNonEmpty } from '~/util';
import { usePlatform, type Platform } from '~/util/Platform';

import { useExplorerContext } from '../Context';
import { getQuickPreviewStore } from '../QuickPreview/store';
import { RevealInNativeExplorerBase } from '../RevealInNativeExplorer';
import { explorerStore } from '../store';
import { useViewItemDoubleClick } from '../View/ViewItem';
import { Conditional, ConditionalItem } from './ConditionalItem';
import { useContextMenuContext } from './context';
import OpenWith from './OpenWith';

export const OpenOrDownload = new ConditionalItem({
	useCondition: () => {
		const { selectedFilePaths, selectedEphemeralPaths } = useContextMenuContext();
		const { openFilePaths, openEphemeralFiles } = usePlatform();

		if (
			!openFilePaths ||
			!openEphemeralFiles ||
			(!isNonEmpty(selectedFilePaths) && !isNonEmpty(selectedEphemeralPaths))
		)
			return null;

		return { openFilePaths, openEphemeralFiles, selectedFilePaths, selectedEphemeralPaths };
	},
	Component: () => {
		const keybind = useKeybindFactory();
		const { platform } = usePlatform();
		const { doubleClick } = useViewItemDoubleClick();
		const os = useOperatingSystem(true);

		if (platform === 'web') return <Menu.Item label="Download" />;
		else
			return (
				<>
					<Menu.Item
						label="Open"
						keybind={keybind(os === 'windows' ? [] : [ModifierKeys.Control], [
							os === 'windows' ? 'Enter' : 'O'
						])}
						onClick={() => doubleClick()}
					/>
					<Conditional items={[OpenWith]} />
				</>
			);
	}
});

export const OpenQuickView = () => {
	const keybind = useKeybindFactory();

	return (
		<ContextMenu.Item
			label="Quick view"
			keybind={keybind([], [' '])}
			onClick={() => (getQuickPreviewStore().open = true)}
		/>
	);
};

export const Details = new ConditionalItem({
	useCondition: () => {
		const showInspector = useSelector(explorerStore, (s) => s.showInspector);
		if (showInspector) return null;

		return {};
	},
	Component: () => {
		const keybind = useKeybindFactory();

		return (
			<ContextMenu.Item
				label="Details"
				keybind={keybind([ModifierKeys.Control], ['I'])}
				// icon={Sidebar}
				onClick={() => (explorerStore.showInspector = true)}
			/>
		);
	}
});

export const Rename = new ConditionalItem({
	useCondition: () => {
		const { selectedItems } = useContextMenuContext();

		const settings = useExplorerContext().useSettingsSnapshot();

		if (settings.layoutMode === 'media' || selectedItems.length > 1) return null;

		return {};
	},
	Component: () => {
		const keybind = useKeybindFactory();
		const os = useOperatingSystem(true);

		return (
			<ContextMenu.Item
				label="Rename"
				keybind={keybind([], [os === 'windows' ? 'F2' : 'Enter'])}
				onClick={() => (explorerStore.isRenaming = true)}
			/>
		);
	}
});

export const RevealInNativeExplorer = new ConditionalItem({
	useCondition: () => {
		const { selectedItems } = useContextMenuContext();

		const items = useMemo(() => {
			const array: Parameters<NonNullable<Platform['revealItems']>>[1] = [];

			for (const item of selectedItems) {
				switch (item.type) {
					case 'Path': {
						array.push({
							FilePath: { id: item.item.id }
						});
						break;
					}
					case 'Object': {
						// this isn't good but it's the current behaviour
						const filePath = item.item.file_paths[0];
						if (filePath)
							array.push({
								FilePath: {
									id: filePath.id
								}
							});
						else return [];
						break;
					}
					case 'Location': {
						array.push({
							Location: {
								id: item.item.id
							}
						});
						break;
					}
					case 'NonIndexedPath': {
						array.push({
							Ephemeral: {
								path: item.item.path
							}
						});
						break;
					}
				}
			}

			return array;
		}, [selectedItems]);

		if (!isNonEmpty(items)) return null;

		return { items };
	},
	Component: ({ items }) => <RevealInNativeExplorerBase items={items} />
});

export const Deselect = new ConditionalItem({
	useCondition: () => {
		const cutCopyState = useSelector(explorerStore, (s) => s.cutCopyState);

		if (cutCopyState.type === 'Idle') return null;

		return {};
	},
	Component: () => (
		<ContextMenu.Item
			label="Deselect"
			icon={FileX}
			onClick={() => {
				explorerStore.cutCopyState = {
					type: 'Idle'
				};
			}}
		/>
	)
});

export const Share = () => {
	return (
		<>
			<Menu.Item
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
