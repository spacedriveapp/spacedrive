import clsx from 'clsx';
import { ArrowsOutSimple } from 'phosphor-react';
import { memo } from 'react';
import { ExplorerItem } from '@sd/client';
import { Button } from '@sd/ui';
import GridList from '~/components/GridList';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { ViewItem } from '.';
import FileThumb from '../File/Thumb';
import { useExplorerViewContext } from '../ViewContext';

interface MediaViewItemProps {
	data: ExplorerItem;
	index: number;
	selected: boolean;
}

const MediaViewItem = memo(({ data, index, selected }: MediaViewItemProps) => {
	const explorerStore = useExplorerStore();

	return (
		<ViewItem
			data={data}
			className={clsx(
				'h-full w-full overflow-hidden border-2',
				selected ? 'border-accent' : 'border-transparent'
			)}
		>
			<div
				className={clsx(
					'group relative flex aspect-square items-center justify-center hover:bg-app-selectedItem',
					selected && 'bg-app-selectedItem'
				)}
			>
				<FileThumb
					size={0}
					data={data}
					cover={explorerStore.mediaAspectSquare}
					className="!rounded-none"
				/>

				<Button
					variant="gray"
					size="icon"
					className="absolute right-2 top-2 hidden rounded-full shadow group-hover:block"
					onClick={() => (getExplorerStore().quickViewObject = data)}
				>
					<ArrowsOutSimple />
				</Button>
			</div>
		</ViewItem>
	);
});

export default () => {
	const explorerStore = useExplorerStore();
	const explorerView = useExplorerViewContext();

	return (
		<GridList
			scrollRef={explorerView.scrollRef}
			count={explorerView.items?.length || 100}
			columns={explorerStore.mediaColumns}
			selected={explorerView.selected}
			onSelectedChange={explorerView.onSelectedChange}
			overscan={explorerView.overscan}
			onLoadMore={explorerView.onLoadMore}
			rowsBeforeLoadMore={explorerView.rowsBeforeLoadMore}
			top={explorerView.top}
			preventSelection={!explorerView.selectable}
			preventContextMenuSelection={!explorerView.contextMenu}
		>
			{({ index, item: Item }) => {
				if (!explorerView.items) {
					return (
						<Item className="!p-px">
							<div className="h-full animate-pulse bg-app-box" />
						</Item>
					);
				}

				const item = explorerView.items[index];
				if (!item) return null;

				const isSelected = Array.isArray(explorerView.selected)
					? explorerView.selected.includes(item.item.id)
					: explorerView.selected === item.item.id;

				return (
					<Item selectable selected={isSelected} index={index} id={item.item.id}>
						<MediaViewItem data={item} index={index} selected={isSelected} />
					</Item>
				);
			}}
		</GridList>
	);
};
