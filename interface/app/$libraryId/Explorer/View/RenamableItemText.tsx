import type { ExplorerItem } from '@sd/client';

import clsx from 'clsx';
import { useCallback, useRef } from 'react';

import {
	getEphemeralPath,
	getExplorerItemData,
	getIndexedItemFilePath,
	useLibraryMutation,
	useRspcLibraryContext,
	useSelector
} from '@sd/client';
import { toast } from '@sd/ui';
import { useIsDark, useLocale } from '~/hooks';

import { useExplorerContext } from '../Context';
import { RenameTextBox, RenameTextBoxProps } from '../FilePath/RenameTextBox';
import { useQuickPreviewStore } from '../QuickPreview/store';
import { explorerStore } from '../store';

type TextBoxProps = Pick<
	RenameTextBoxProps,
	'toggleBy' | 'lines' | 'editLines' | 'className' | 'idleClassName' | 'activeClassName' | 'style'
>;

interface Props extends TextBoxProps {
	item: ExplorerItem;
	selected?: boolean;
	highlight?: boolean;
	allowHighlight?: boolean;
}

const RENAMABLE_ITEM_TYPES: Partial<Record<ExplorerItem['type'], boolean>> = {
	Location: true,
	Path: true,
	NonIndexedPath: true,
	Object: true
};

export const RenamableItemText = ({
	item,
	selected,
	highlight,
	className,
	allowHighlight = true,
	...props
}: Props) => {
	const isDark = useIsDark();
	const rspc = useRspcLibraryContext();

	const explorer = useExplorerContext({ suspense: false });
	const isDragging = useSelector(explorerStore, s => s.drag?.type === 'dragging');

	const quickPreviewStore = useQuickPreviewStore();

	const itemData = getExplorerItemData(item);

	const ref = useRef<HTMLDivElement>(null);

	const renameFile = useLibraryMutation(['files.renameFile'], {
		onError: () => reset(),
		onSuccess: () => rspc.queryClient.invalidateQueries({ queryKey: ['search.paths'] })
	});

	const renameEphemeralFile = useLibraryMutation(['ephemeralFiles.renameFile'], {
		onError: () => reset(),
		onSuccess: () => rspc.queryClient.invalidateQueries({ queryKey: ['search.paths'] })
	});

	const renameLocation = useLibraryMutation(['locations.update'], {
		onError: () => reset(),
		onSuccess: () => rspc.queryClient.invalidateQueries({ queryKey: ['search.paths'] })
	});

	const reset = useCallback(() => {
		if (!ref.current || !itemData.fullName) return;
		ref.current.innerText = itemData.fullName;
	}, [itemData.fullName]);

	const { t } = useLocale();

	const handleRename = useCallback(
		async (newName: string) => {
			try {
				switch (item.type) {
					case 'Location': {
						const locationId = item.item.id;
						if (!locationId) throw new Error('Missing location id');

						await renameLocation.mutateAsync({
							id: locationId,
							path: null,
							name: newName,
							generate_preview_media: null,
							sync_preview_media: null,
							hidden: null,
							indexer_rules_ids: []
						});

						break;
					}

					case 'Path':
					case 'Object': {
						const filePathData = getIndexedItemFilePath(item);

						if (!filePathData) throw new Error('Failed to get file path object');

						const { id, location_id } = filePathData;

						if (!location_id) throw new Error('Missing location id');

						await renameFile.mutateAsync({
							location_id: location_id,
							kind: {
								One: {
									from_file_path_id: id,
									to: newName
								}
							}
						});

						break;
					}

					case 'NonIndexedPath': {
						const ephemeralFile = getEphemeralPath(item);

						if (!ephemeralFile) throw new Error('Failed to get ephemeral file object');

						renameEphemeralFile.mutate({
							kind: {
								One: {
									from_path: ephemeralFile.path,
									to: newName
								}
							}
						});

						break;
					}

					default:
						throw new Error('Invalid explorer item type');
				}
			} catch (e) {
				reset();
				toast.error({
					title: t('failed_to_rename_file', {
						oldName: itemData.fullName,
						newName
					}),
					body: t('error_message', { error: e })
				});
			}
		},
		[itemData.fullName, item, renameEphemeralFile, renameFile, renameLocation, reset, t]
	);

	const disabled =
		!selected ||
		isDragging ||
		!explorer ||
		explorer.selectedItems.size > 1 ||
		quickPreviewStore.open ||
		!RENAMABLE_ITEM_TYPES[item.type];

	return (
		<RenameTextBox
			name={itemData.fullName ?? itemData.name ?? ''}
			disabled={disabled}
			onRename={handleRename}
			className={clsx(
				'font-medium',
				className,
				(selected || highlight) && allowHighlight && ['bg-accent', !isDark && 'text-white']
			)}
			{...props}
		/>
	);
};
