import clsx from 'clsx';
import { memo } from 'react';
import { type ExplorerItem, byteSize, getItemFilePath, getItemLocation } from '@sd/client';
import { ViewItem } from '.';
import { useExplorerContext } from '../Context';
import { FileThumb } from '../FilePath/Thumb';
import { useExplorerViewContext } from '../ViewContext';
import GridList from './GridList';
import RenamableItemText from './RenamableItemText';

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

	const showSize =
		!filePathData?.is_dir &&
		!location &&
		showBytesInGridView &&
		(!isRenaming || (isRenaming && !selected));

	return (
		<ViewItem data={data} className="h-full w-full">
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
				<RenamableItemText
					item={data}
					selected={selected}
					style={{ maxHeight: gridItemSize / 3 }}
					disabled={!renamable}
				/>
				{showSize && filePathData?.size_in_bytes_bytes && (
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

	return (
		<GridList>
			{({ item, selected, cut }) => (
				<GridViewItem
					data={item}
					selected={selected}
					cut={cut}
					isRenaming={explorerView.isRenaming}
					renamable={explorer.selectedItems.size === 1}
				/>
			)}
		</GridList>
	);
};
