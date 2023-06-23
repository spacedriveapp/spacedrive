import byteSize from 'byte-size';
import clsx from 'clsx';
import { memo } from 'react';
import { ExplorerItem, bytesToNumber, getItemFilePath, getItemLocation } from '@sd/client';
import GridList from '~/components/GridList';
import { isCut, useExplorerStore } from '~/hooks';
import { ViewItem } from '.';
import FileThumb from '../File/Thumb';
import { useExplorerViewContext } from '../ViewContext';
import RenamableItemText from './RenamableItemText';

interface GridViewItemProps {
	data: ExplorerItem;
	selected: boolean;
	index: number;
	cut: boolean;
}

const GridViewItem = memo(({ data, selected, index, cut, ...props }: GridViewItemProps) => {
	const filePathData = getItemFilePath(data);
	const location = getItemLocation(data);
	const explorerStore = useExplorerStore();
	const explorerView = useExplorerViewContext();

	const showSize =
		!filePathData?.is_dir &&
		!location &&
		explorerStore.showBytesInGridView &&
		(!explorerView.isRenaming || (explorerView.isRenaming && !selected));

	return (
		<ViewItem data={data} className="h-full w-full" {...props}>
			<div className={clsx('mb-1 rounded-lg ', selected && 'bg-app-selectedItem')}>
				<FileThumb
					data={data}
					size={explorerStore.gridItemSize}
					className={clsx('mx-auto', cut && 'opacity-60')}
				/>
			</div>

			<div className="flex flex-col justify-center">
				<RenamableItemText item={data} selected={selected} />
				{showSize && filePathData?.size_in_bytes_bytes && (
					<span
						className={clsx(
							'cursor-default truncate rounded-md px-1.5 py-[1px] text-center text-tiny text-ink-dull '
						)}
					>
						{byteSize(bytesToNumber(filePathData.size_in_bytes_bytes)).toString()}
					</span>
				)}
			</div>
		</ViewItem>
	);
});

export default () => {
	const explorerStore = useExplorerStore();
	const explorerView = useExplorerViewContext();

	const itemDetailsHeight =
		explorerStore.gridItemSize / 4 + (explorerStore.showBytesInGridView ? 20 : 0);
	const itemHeight = explorerStore.gridItemSize + itemDetailsHeight;

	return (
		<GridList
			scrollRef={explorerView.scrollRef}
			count={explorerView.items?.length || 100}
			size={{ width: explorerStore.gridItemSize, height: itemHeight }}
			padding={12}
			selectable={!!explorerView.items}
			selected={explorerView.selected}
			onSelectedChange={explorerView.onSelectedChange}
			overscan={explorerView.overscan}
			onLoadMore={explorerView.onLoadMore}
			rowsBeforeLoadMore={explorerView.rowsBeforeLoadMore}
			top={explorerView.top}
			preventSelection={explorerView.isRenaming || !explorerView.selectable}
			preventContextMenuSelection={!explorerView.contextMenu}
		>
			{({ index, item: Item }) => {
				const item = explorerView.items?.[index];
				if (!item) return null;

				const isSelected = Array.isArray(explorerView.selected)
					? explorerView.selected.includes(item.item.id)
					: explorerView.selected === item.item.id;

				const cut = isCut(item.item.id);

				return (
					<Item selected={isSelected} id={item.item.id}>
						<GridViewItem data={item} selected={isSelected} index={index} cut={cut} />
					</Item>
				);
			}}
		</GridList>
	);
};
