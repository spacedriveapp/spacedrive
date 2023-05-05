import { useVirtualizer } from '@tanstack/react-virtual';
import clsx from 'clsx';
import { memo, useEffect, useMemo, useRef, useState } from 'react';
import { useKey, useOnWindowResize } from 'rooks';
import { ExplorerItem, formatBytes } from '@sd/client';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import RenameTextBox from './File/RenameTextBox';
import Thumb from './File/Thumb';
import { ViewItem } from './View';
import { useExplorerViewContext } from './ViewContext';
import { getItemFilePath } from './util';

interface GridViewItemProps {
	data: ExplorerItem;
	selected: boolean;
	index: number;
}

const GridViewItem = memo(({ data, selected, index, ...props }: GridViewItemProps) => {
	const filePathData = data ? getItemFilePath(data) : null;
	const explorerStore = useExplorerStore();

	return (
		<ViewItem
			data={data}
			index={index}
			draggable
			style={{ width: explorerStore.gridItemSize }}
			{...props}
		>
			<div
				style={{
					width: explorerStore.gridItemSize,
					height: explorerStore.gridItemSize
				}}
				className={clsx(
					'mb-1 flex items-center justify-center justify-items-center rounded-lg border-2 border-transparent text-center active:translate-y-[1px]',
					{
						'bg-app-selected/20': selected
					}
				)}
			>
				<Thumb data={data} size={explorerStore.gridItemSize} />
			</div>
			<div className="flex flex-col justify-center">
				{filePathData && (
					<RenameTextBox
						filePathData={filePathData}
						selected={selected}
						className={clsx(
							'text-center font-medium',
							selected && 'bg-accent text-white'
						)}
						style={{
							maxHeight: explorerStore.gridItemSize / 3
						}}
					/>
				)}
				{explorerStore.showBytesInGridView &&
					(!explorerStore.isRenaming || (explorerStore.isRenaming && !selected)) && (
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
	const { data, scrollRef, onLoadMore, hasNextPage, isFetchingNextPage } =
		useExplorerViewContext();

	const [width, setWidth] = useState(0);

	// Virtualizer count calculation
	const amountOfColumns = Math.floor(width / explorerStore.gridItemSize) || 1;
	const amountOfRows = Math.ceil(data.length / amountOfColumns);

	// Virtualizer item size calculation
	const gridTextAreaHeight =
		explorerStore.gridItemSize / 4 + (explorerStore.showBytesInGridView ? 20 : 0);
	const itemSize = explorerStore.gridItemSize + gridTextAreaHeight;

	const rowVirtualizer = useVirtualizer({
		count: amountOfRows,
		getScrollElement: () => scrollRef.current,
		estimateSize: () => itemSize,
		measureElement: () => itemSize,
		paddingStart: 12,
		paddingEnd: 12
	});

	const virtualRows = rowVirtualizer.getVirtualItems();

	useEffect(() => {
		const lastRow = virtualRows[virtualRows.length - 1];
		if (lastRow?.index === amountOfRows - 1 && hasNextPage && !isFetchingNextPage) {
			onLoadMore?.();
		}
	}, [hasNextPage, onLoadMore, isFetchingNextPage, virtualRows, data.length]);

	function handleWindowResize() {
		if (scrollRef.current) {
			setWidth(scrollRef.current.offsetWidth);
		}
	}

	// Resize view on initial render
	useEffect(() => handleWindowResize(), []);

	// Resize view on window resize
	useOnWindowResize(handleWindowResize);

	const lastSelectedIndex = useRef(explorerStore.selectedRowIndex);

	// Resize view on item selection/deselection
	useEffect(() => {
		const { selectedRowIndex } = explorerStore;

		if (
			explorerStore.showInspector &&
			typeof lastSelectedIndex.current !== typeof selectedRowIndex
		) {
			handleWindowResize();
		}

		lastSelectedIndex.current = selectedRowIndex;
	}, [explorerStore.selectedRowIndex]);

	// Resize view on inspector toggle
	useEffect(() => {
		if (explorerStore.selectedRowIndex !== null) handleWindowResize();
	}, [explorerStore.showInspector]);

	// Measure item on grid item size change
	useEffect(() => {
		rowVirtualizer.measure();
	}, [explorerStore.showBytesInGridView, explorerStore.gridItemSize, rowVirtualizer]);

	// Force recalculate range
	// https://github.com/TanStack/virtual/issues/485
	useMemo(() => {
		// @ts-ignore
		rowVirtualizer.calculateRange();
	}, [amountOfRows, rowVirtualizer]);

	// Select item with arrow up key
	useKey(
		'ArrowUp',
		(e) => {
			e.preventDefault();

			const { selectedRowIndex } = explorerStore;

			if (selectedRowIndex === null) return;

			getExplorerStore().selectedRowIndex = Math.max(selectedRowIndex - 1, 0);
		},
		{ when: !explorerStore.isRenaming }
	);

	// Select item with arrow down key
	useKey(
		'ArrowDown',
		(e) => {
			e.preventDefault();

			const { selectedRowIndex } = explorerStore;

			if (selectedRowIndex === null) return;

			getExplorerStore().selectedRowIndex = Math.min(selectedRowIndex + 1, data.length - 1);
		},
		{ when: !explorerStore.isRenaming }
	);

	if (!width) return null;

	return (
		<div
			className="relative"
			style={{
				height: `${rowVirtualizer.getTotalSize()}px`
			}}
		>
			{virtualRows.map((virtualRow) => (
				<div
					key={virtualRow.key}
					className="absolute left-0 top-0 flex w-full"
					style={{
						height: virtualRow.size,
						transform: `translateY(${virtualRow.start}px)`
					}}
				>
					{[...Array(amountOfColumns)].map((_, i) => {
						const index = virtualRow.index * amountOfColumns + i;
						const item = data[index];
						const isSelected = explorerStore.selectedRowIndex === index;

						if (!item) return null;
						return (
							<GridViewItem
								key={item.item.id}
								data={item}
								selected={isSelected}
								index={index}
							/>
						);
					})}
				</div>
			))}
		</div>
	);
};
