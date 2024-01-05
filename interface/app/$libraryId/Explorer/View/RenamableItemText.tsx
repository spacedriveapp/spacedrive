import clsx from 'clsx';
import { memo, useMemo, useRef } from 'react';
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
import { RenameTextBox, RenameTextBoxProps } from '../FilePath/RenameTextBox';
import { useQuickPreviewStore } from '../QuickPreview/store';
import { useExplorerStore } from '../store';

interface Props extends Pick<RenameTextBoxProps, 'idleClassName' | 'lines'> {
	item: ExplorerItem;
	allowHighlight?: boolean;
	style?: React.CSSProperties;
	highlight?: boolean;
	selected?: boolean;
}

type InnerProps = Omit<Props, 'item'> & {
	data: ReturnType<typeof useDerivedData>;
	defaultDisabled: boolean;
};

// We derive the exact data required by `RenamableItemTextInner` here.
// We do this outside so we can use `useMemo` and avoid unnecessary re-renders.
//
// This function acts as a selector so the component only rerenders when properties we care about change.
function deriveItemData(item: ExplorerItem) {
	if (item.type === 'Location') {
		return {
			type: item.type,
			locationId: item.item.id
		};
	} else if (item.type === 'Path' || item.type === 'Object') {
		const filePathData = getIndexedItemFilePath(item);
		return {
			type: item.type,
			id: filePathData?.id,
			locationId: filePathData?.location_id
		};
	} else if (item.type === 'NonIndexedPath') {
		const ephemeralFile = getEphemeralPath(item);
		return {
			type: item.type,
			path: ephemeralFile?.path
		};
	} else {
		return {
			type: item.type
		};
	}
}

function useDerivedData(item: ExplorerItem) {
	// We use `JSON.stringify` to ensure referential integrity. // TODO: I'm sure a better way to do this exists.
	return useMemo(() => {
		const itemData = getExplorerItemData(item);
		return {
			fullName: itemData.fullName,
			name: itemData.name,
			data: deriveItemData(item)
		};
	}, [JSON.stringify(item)]);
}

// We break this out so that the component only rerenders when properties we care about change.
//
// Eg. the Dnd context changes a lot but most changes to it don't affect the specific condition this component cares about.
export function RenamableItemText({ item, ...props }: Props) {
	const explorerStore = useExplorerStore();

	return (
		<RenamableItemTextInner
			{...props}
			data={useDerivedData(item)}
			defaultDisabled={explorerStore.drag?.type === 'dragging'}
		/>
	);
}

const RenamableItemTextInner = memo(({ allowHighlight = true, data, ...props }: InnerProps) => {
	const isDark = useIsDark();
	const rspc = useRspcLibraryContext();

	const explorer = useExplorerContext({ suspense: false });

	const quickPreviewStore = useQuickPreviewStore();

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
		if (!ref.current || !data.fullName) return;
		ref.current.innerText = data.fullName;
	};

	const handleRename = async (newName: string) => {
		try {
			switch (data.data.type) {
				case 'Location': {
					const { locationId } = data.data;
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
					const { id, locationId } = data.data;
					if (!id || !locationId) throw new Error('Failed to get file path object');
					await renameFile.mutateAsync({
						location_id: locationId,
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
					const { path } = data.data;
					if (!path) throw new Error('Failed to get ephemeral file object');
					renameEphemeralFile.mutate({
						kind: {
							One: {
								from_path: path,
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
				title: `Could not rename ${data.fullName} to ${newName}`,
				body: `Error: ${e}.`
			});
		}
	};

	const disabled =
		!props.selected ||
		props.defaultDisabled ||
		!explorer ||
		explorer.selectedItems.size > 1 ||
		quickPreviewStore.open ||
		data.data.type === 'SpacedropPeer';

	return (
		<RenameTextBox
			name={data.fullName ?? data.name ?? ''}
			disabled={disabled}
			onRename={handleRename}
			className={clsx(
				'font-medium',
				(props.selected || props.highlight) &&
					allowHighlight && ['bg-accent', !isDark && 'text-white']
			)}
			style={props.style}
			lines={props.lines}
			idleClassName={props.idleClassName}
		/>
	);
});
