import clsx from 'clsx';
import { useMemo, useRef } from 'react';
import {
	getExplorerItemData,
	getItemFilePath,
	useLibraryMutation,
	useRspcLibraryContext,
	type ExplorerItem
} from '@sd/client';
import { toast } from '@sd/ui';
import { useIsDark } from '~/hooks';

import { useExplorerContext } from '../Context';
import { RenameTextBox } from '../FilePath/RenameTextBox';
import { useQuickPreviewStore } from '../QuickPreview/store';

interface Props {
	item: ExplorerItem;
	allowHighlight?: boolean;
	style?: React.CSSProperties;
	lines?: number;
}

export const RenamableItemText = ({ item, allowHighlight = true, style, lines }: Props) => {
	const rspc = useRspcLibraryContext();
	const explorer = useExplorerContext();
	const quickPreviewStore = useQuickPreviewStore();
	const isDark = useIsDark();

	const itemData = getExplorerItemData(item);

	const ref = useRef<HTMLDivElement>(null);

	const selected = useMemo(
		() => explorer.selectedItems.has(item),
		[explorer.selectedItems, item]
	);

	const renameFile = useLibraryMutation(['files.renameFile'], {
		onError: () => reset(),
		onSuccess: () => rspc.queryClient.invalidateQueries(['search.paths'])
	});

	const renameLocation = useLibraryMutation(['locations.update'], {
		onError: () => reset(),
		onSuccess: () => rspc.queryClient.invalidateQueries(['search.paths'])
	});

	const reset = () => {
		if (!ref.current || !itemData.fullName) return;
		ref.current.innerText = itemData.fullName;
	};

	const handleRename = async (newName: string) => {
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

				default: {
					const filePathData = getItemFilePath(item);

					if (!filePathData || !('id' in filePathData))
						throw new Error('Unable to rename file');

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
				}
			}
		} catch (e) {
			reset();
			toast.error({
				title: `Could not rename ${itemData.fullName} to ${newName}`,
				body: `Error: ${e}.`
			});
		}
	};

	const disabled =
		!selected ||
		explorer.selectedItems.size > 1 ||
		quickPreviewStore.open ||
		item.type === 'NonIndexedPath' ||
		item.type === 'SpacedropPeer';

	return (
		<RenameTextBox
			name={itemData.fullName ?? itemData.name ?? ''}
			disabled={disabled}
			onRename={handleRename}
			className={clsx(
				'text-center font-medium',
				selected && allowHighlight && ['bg-accent', !isDark && 'text-white']
			)}
			style={style}
			lines={lines}
		/>
	);
};
