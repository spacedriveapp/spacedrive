import clsx from 'clsx';
import { memo } from 'react';
import { stringify } from 'uuid';
import { ExplorerItem, byteSize, getItemFilePath, getItemLocation } from '@sd/client';
import { GridList } from '~/components';
import { ViewItem } from '.';
import { useExplorerContext } from '../Context';
import FileThumb from '../FilePath/Thumb';
import { useExplorerViewContext } from '../ViewContext';
import { isCut, useExplorerStore } from '../store';
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
	const explorerContext = useExplorerContext();
	const locationUuid =
		explorerContext.parent?.type === 'Location'
			? stringify(explorerContext.parent.location.pub_id)
			: '';
	const explorerSettings =
		explorerStore.viewLocationPreferences?.location?.[locationUuid]?.explorer;
	const itemSize = explorerSettings?.itemSize ?? explorerStore.gridItemSize;

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
					size={itemSize}
					className={clsx('mx-auto', cut && 'opacity-60')}
				/>
			</div>

			<div className="flex flex-col justify-center">
				<RenamableItemText
					item={data}
					selected={selected}
					style={{ maxHeight: itemSize / 3 }}
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
	const explorerStore = useExplorerStore();
	const explorerView = useExplorerViewContext();
	const explorerContext = useExplorerContext();
	const locationUuid =
		explorerContext.parent?.type === 'Location'
			? stringify(explorerContext.parent.location.pub_id)
			: '';
	const explorerSettings =
		explorerStore.viewLocationPreferences?.location?.[locationUuid]?.explorer;
	const itemSize = explorerSettings?.itemSize ?? explorerStore.gridItemSize;

	const itemDetailsHeight = itemSize / 4 + (explorerStore.showBytesInGridView ? 20 : 0);
	const itemHeight = itemSize + itemDetailsHeight;

	return (
		<GridList
			scrollRef={explorerView.scrollRef}
			count={explorerView.items?.length || 100}
			size={{ width: itemSize, height: itemHeight }}
			padding={12}
			selectable={!!explorerView.items}
			selected={explorerView.selected}
			onSelectedChange={explorerView.onSelectedChange}
			overscan={explorerView.overscan}
			onLoadMore={explorerView.onLoadMore}
			rowsBeforeLoadMore={explorerView.rowsBeforeLoadMore}
			top={explorerView.top}
			preventSelection={explorerView.isRenaming || !explorerView.selectable}
			preventContextMenuSelection={explorerView.contextMenu === undefined}
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
