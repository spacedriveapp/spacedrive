import { FileX, Share as ShareIcon } from 'phosphor-react';
import { useMemo } from 'react';
import { ContextMenu, ModifierKeys } from '@sd/ui';
import { useKeybindFactory } from '~/hooks/useKeybindFactory';
import { isNonEmpty } from '~/util';
import { type Platform } from '~/util/Platform';
import { useExplorerContext } from '../Context';
import { RevealInNativeExplorerBase } from '../RevealInNativeExplorer';
import { useExplorerViewContext } from '../ViewContext';
import { getExplorerStore, useExplorerStore } from '../store';
import { ConditionalItem } from './ConditionalItem';
import { useContextMenuContext } from './context';

export const OpenQuickView = () => {
	const keybind = useKeybindFactory();
	const { selectedItems } = useContextMenuContext();

	return (
		<ContextMenu.Item
			label="Quick view"
			keybind={keybind([], [' '])}
			onClick={() =>
				// using [0] is not great
				(getExplorerStore().quickViewObject = selectedItems[0])
			}
		/>
	);
};

export const Details = new ConditionalItem({
	useCondition: () => {
		const { showInspector } = useExplorerStore();
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
				onClick={() => (getExplorerStore().showInspector = true)}
			/>
		);
	}
});

export const Rename = new ConditionalItem({
	useCondition: () => {
		const { selectedItems } = useContextMenuContext();

		const settings = useExplorerContext().useSettingsSnapshot();

		if (
			settings.layoutMode === 'media' ||
			selectedItems.length > 1 ||
			selectedItems.some((item) => item.type === 'NonIndexedPath')
		)
			return null;

		return {};
	},
	Component: () => {
		const explorerView = useExplorerViewContext();
		const keybind = useKeybindFactory();

		return (
			<ContextMenu.Item
				label="Rename"
				keybind={keybind([], ['Enter'])}
				onClick={() => explorerView.setIsRenaming(true)}
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
		const { cutCopyState } = useExplorerStore();

		if (cutCopyState.type === 'Idle') return null;

		return {};
	},
	Component: () => (
		<ContextMenu.Item
			label="Deselect"
			icon={FileX}
			onClick={() => {
				getExplorerStore().cutCopyState = {
					type: 'Idle'
				};
			}}
		/>
	)
});

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
