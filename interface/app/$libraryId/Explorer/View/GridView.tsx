import clsx from 'clsx';
import { memo } from 'react';
import { useMatch } from 'react-router';
import { byteSize, getItemFilePath, getItemLocation, type ExplorerItem } from '@sd/client';

import { useExplorerContext } from '../Context';
import { FileThumb } from '../FilePath/Thumb';
import { useQuickPreviewStore } from '../QuickPreview/store';
import { useExplorerViewContext } from '../ViewContext';
import GridList from './GridList';
import { RenamableItemText } from './RenamableItemText';
import { ViewItem } from './ViewItem';

interface GridViewItemProps {
	data: ExplorerItem;
	selected: boolean;
	isRenaming: boolean;
	cut: boolean;
	renamable: boolean;
}

const GridViewItem = memo(({ data, selected, cut, isRenaming, renamable }: GridViewItemProps) => {
	const explorer = useExplorerContext();
	const { showBytesInGridView, gridItemSize } = explorer.useSettingsSnapshot();

	const filePathData = getItemFilePath(data);
	const location = getItemLocation(data);
	const isEphemeralLocation = useMatch('/:libraryId/ephemeral/:ephemeralId');
	const isFolder = 'is_dir' in data.item ? data.item.is_dir || data.type === 'Location' : false;

	//do not refactor please - this has been done for readability

	const shouldShowSize = () => {
		if (isEphemeralLocation) return false;
		if (isFolder) return false;
		if (!filePathData?.is_dir && !location) return false;
		if (showBytesInGridView) return true;
		if (isRenaming) return false;
		if (!selected) return false;

		return true;
	};

	return (
		<ViewItem data={data} className="w-full h-full">
			<div
				className={clsx('mb-1 aspect-square rounded-lg', selected && 'bg-app-selectedItem')}
			>
				<FileThumb
					data={data}
					frame
					blackBars
					extension
					className={clsx('px-2 py-1', cut && 'opacity-60')}
				/>
			</div>

			<div className="flex flex-col justify-center">
				<RenamableItemText item={data} style={{ maxHeight: gridItemSize / 3 }} />
				{shouldShowSize() && filePathData?.size_in_bytes_bytes && (
					<span
						className={clsx(
							'cursor-default truncate rounded-md px-1.5 py-[1px] text-center text-tiny text-ink-dull '
						)}
					>
						{`${byteSize(filePathData.size_in_bytes_bytes)}`}
					</span>
				)}
			</div>
		</ViewItem>
	);
});

export default () => {
	const explorer = useExplorerContext();
	const explorerView = useExplorerViewContext();
	const quickPreviewStore = useQuickPreviewStore();

	return (
		<GridList>
			{({ item, selected, cut }) => (
				<GridViewItem
					data={item}
					selected={selected}
					cut={cut}
					isRenaming={explorerView.isRenaming}
					renamable={
						selected && explorer.selectedItems.size === 1 && !quickPreviewStore.open
					}
				/>
			)}
		</GridList>
	);
};
