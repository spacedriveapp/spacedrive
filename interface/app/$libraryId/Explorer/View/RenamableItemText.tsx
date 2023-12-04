import clsx from 'clsx';
import { useRef } from 'react';
import {
	getEphemeralPath,
	getExplorerItemData,
	getIndexedItemFilePath,
	useLibraryMutation,
	useRspcLibraryContext,
	type ExplorerItem
} from '@sd/client';
import { toast } from '@sd/ui';
import { useIsDark } from '~/hooks';

import { useExplorerContext } from '../Context';
import { RenameTextBox } from '../FilePath/RenameTextBox';
import { useQuickPreviewStore } from '../QuickPreview/store';
import { useExplorerStore } from '../store';

interface Props {
	item: ExplorerItem;
	allowHighlight?: boolean;
	style?: React.CSSProperties;
	lines?: number;
	highlight?: boolean;
	selected?: boolean;
}

export const RenamableItemText = ({ allowHighlight = true, ...props }: Props) => {
	const isDark = useIsDark();
	const rspc = useRspcLibraryContext();

	const explorer = useExplorerContext({ suspense: false });
	const explorerStore = useExplorerStore();

	const quickPreviewStore = useQuickPreviewStore();

	const itemData = getExplorerItemData(props.item);

	const ref = useRef<HTMLDivElement>(null);

	const renameFile = useLibraryMutation(['files.renameFile'], {
		onError: () => reset(),
		onSuccess: () => rspc.queryClient.invalidateQueries(['search.paths'])
	});

	const renameEphemeralFile = useLibraryMutation(['ephemeralFiles.renameFile'], {
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
			switch (props.item.type) {
				case 'Location': {
					const locationId = props.item.item.id;
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
					const filePathData = getIndexedItemFilePath(props.item);

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
					const ephemeralFile = getEphemeralPath(props.item);

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
				title: `Could not rename ${itemData.fullName} to ${newName}`,
				body: `Error: ${e}.`
			});
		}
	};

	const disabled =
		!props.selected ||
		explorerStore.drag?.type === 'dragging' ||
		!explorer ||
		explorer.selectedItems.size > 1 ||
		quickPreviewStore.open ||
		props.item.type === 'SpacedropPeer';

	return (
		<RenameTextBox
			name={itemData.fullName ?? itemData.name ?? ''}
			disabled={disabled}
			onRename={handleRename}
			className={clsx(
				'font-medium',
				(props.selected || props.highlight) &&
					allowHighlight && ['bg-accent', !isDark && 'text-white']
			)}
			style={props.style}
			lines={props.lines}
		/>
	);
};
