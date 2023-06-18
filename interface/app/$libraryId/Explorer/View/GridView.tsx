import clsx from 'clsx';
import { memo } from 'react';
import { ExplorerItem, formatBytes } from '@sd/client';
import GridList from '~/components/GridList';
import { useExplorerStore } from '~/hooks/useExplorerStore';
import { ViewItem } from '.';
import RenameTextBox from '../File/RenameTextBox';
import FileThumb from '../File/Thumb';
import { useExplorerViewContext } from '../ViewContext';
import { getItemFilePath } from '../util';

interface GridViewItemProps {
	data: ExplorerItem;
	selected: boolean;
	index: number;
}

const GridViewItem = memo(({ data, selected, index, ...props }: GridViewItemProps) => {
	const filePathData = data ? getItemFilePath(data) : null;
	const explorerStore = useExplorerStore();
	const explorerView = useExplorerViewContext();

	return (
		<ViewItem data={data} className="h-full w-full" {...props}>
			<div className={clsx('mb-1 rounded-lg ', selected && 'bg-app-selectedItem')}>
				<FileThumb data={data} size={explorerStore.gridItemSize} className="mx-auto" />
			</div>

			<div className="flex flex-col justify-center">
				{filePathData && (
					<RenameTextBox
						filePathData={filePathData}
						disabled={!selected}
						className={clsx(
							'text-center font-medium text-ink',
							selected && 'bg-accent text-white dark:text-ink'
						)}
						style={{
							maxHeight: explorerStore.gridItemSize / 3
						}}
						activeClassName="!text-ink"
					/>
				)}
				{explorerStore.showBytesInGridView &&
					(!explorerView.isRenaming || (explorerView.isRenaming && !selected)) && (
						<span
							className={clsx(
								'cursor-default truncate rounded-md px-1.5 py-[1px] text-center text-tiny text-ink-dull '
							)}
						>
							{formatBytes(Number(filePathData?.size_in_bytes || 0))}
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
				if (!explorerView.items) {
					return (
						<Item className="p-px">
							<div className="aspect-square animate-pulse rounded-md bg-app-box" />
							<div className="mx-2 mt-3 h-2 animate-pulse rounded bg-app-box" />
							{explorerStore.showBytesInGridView && (
								<div className="mx-8 mt-2 h-1 animate-pulse rounded bg-app-box" />
							)}
						</Item>
					);
				}

				const item = explorerView.items[index];
				if (!item) return null;

				const isSelected = Array.isArray(explorerView.selected)
					? explorerView.selected.includes(item.item.id)
					: explorerView.selected === item.item.id;

				return (
					<Item selected={isSelected} id={item.item.id}>
						<GridViewItem data={item} selected={isSelected} index={index} />
					</Item>
				);
			}}
		</GridList>
	);
};
