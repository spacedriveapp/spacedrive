import { useVirtualizer } from '@tanstack/react-virtual';
import clsx from 'clsx';
import { ArrowsOutSimple } from 'phosphor-react';
import { memo, useEffect, useLayoutEffect, useMemo, useState } from 'react';
import React from 'react';
import { useKey, useOnWindowResize } from 'rooks';
import { ExplorerItem } from '@sd/client';
import { Button } from '@sd/ui';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import Thumb from './File/Thumb';
import { ViewItem, useExplorerView } from './View';

interface MediaViewItemProps {
	data: ExplorerItem;
	index: number;
}

const MediaViewItem = memo(({ data, index }: MediaViewItemProps) => {
	const explorerStore = useExplorerStore();
	const selected = explorerStore.selectedRowIndex === index;

	return (
		<ViewItem
			data={data}
			index={index}
			className={clsx(
				'h-full w-full overflow-hidden border-2 border-transparent',
				selected && 'border-accent'
			)}
		>
			<div
				className={clsx(
					'group relative flex aspect-square items-center justify-center hover:bg-app-selected/20',
					selected && 'bg-app-selected/20'
				)}
			>
				<Thumb
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
	const { data, scrollRef } = useExplorerView();

	const gridPadding = 2;
	const scrollBarWidth = 6;

	const [width, setWidth] = useState(0);
	const [lastSelectedIndex, setLastSelectedIndex] = useState(explorerStore.selectedRowIndex);

	// Virtualizer count calculation
	const amountOfColumns = explorerStore.mediaColumns;
	const amountOfRows = Math.ceil(data.length / amountOfColumns);

	// Virtualizer item size calculation
	const itemSize = (width - gridPadding * 2 - scrollBarWidth) / amountOfColumns;

	const rowVirtualizer = useVirtualizer({
		count: amountOfRows,
		getScrollElement: () => scrollRef.current,
		estimateSize: () => (itemSize < 0 ? 0 : itemSize),
		measureElement: () => itemSize,
		paddingStart: gridPadding,
		paddingEnd: gridPadding
	});

	const columnVirtualizer = useVirtualizer({
		horizontal: true,
		count: amountOfColumns,
		getScrollElement: () => scrollRef.current,
		estimateSize: () => (itemSize < 0 ? 0 : itemSize),
		measureElement: () => itemSize,
		paddingStart: gridPadding,
		paddingEnd: gridPadding
	});

	function handleWindowResize() {
		if (scrollRef.current) {
			setWidth(scrollRef.current.offsetWidth);
		}
	}

	// Resize view on initial render and reset selected item
	useEffect(() => {
		handleWindowResize();
		getExplorerStore().selectedRowIndex = -1;
		return () => {
			getExplorerStore().selectedRowIndex = -1;
		};
	}, []);

	// Resize view on window resize
	useOnWindowResize(handleWindowResize);

	// Resize view on item selection/deselection
	useEffect(() => {
		const index = explorerStore.selectedRowIndex;
		if (
			explorerStore.showInspector &&
			((lastSelectedIndex === -1 && index !== -1) ||
				(lastSelectedIndex !== -1 && index === -1))
		) {
			handleWindowResize();
		}
		setLastSelectedIndex(index);
	}, [explorerStore.selectedRowIndex]);

	// Resize view on inspector toggle
	useEffect(() => {
		if (explorerStore.selectedRowIndex !== -1) {
			handleWindowResize();
		}
	}, [explorerStore.showInspector]);

	// Measure virtual item on size change
	useEffect(() => {
		rowVirtualizer.measure();
		columnVirtualizer.measure();
	}, [rowVirtualizer, columnVirtualizer, itemSize]);

	// Force recalculate range
	// https://github.com/TanStack/virtual/issues/485
	useMemo(() => {
		// @ts-ignore
		rowVirtualizer.calculateRange();
		// @ts-ignore
		columnVirtualizer.calculateRange();
	}, [amountOfRows, amountOfColumns, rowVirtualizer, columnVirtualizer]);

	// Select item with arrow up key
	useKey('ArrowUp', (e) => {
		e.preventDefault();
		if (explorerStore.selectedRowIndex > 0) {
			getExplorerStore().selectedRowIndex = explorerStore.selectedRowIndex - 1;
		}
	});

	// Select item with arrow down key
	useKey('ArrowDown', (e) => {
		e.preventDefault();
		if (
			explorerStore.selectedRowIndex !== -1 &&
			explorerStore.selectedRowIndex !== (data.length ?? 1) - 1
		) {
			getExplorerStore().selectedRowIndex = explorerStore.selectedRowIndex + 1;
		}
	});

	if (!width) return null;
	return (
		<div
			className="relative"
			style={{
				height: `${rowVirtualizer.getTotalSize()}px`,
				width: `${columnVirtualizer.getTotalSize()}px`,
				position: 'relative'
			}}
		>
			{rowVirtualizer.getVirtualItems().map((virtualRow) => (
				<React.Fragment key={virtualRow.index}>
					{columnVirtualizer.getVirtualItems().map((virtualColumn, i) => {
						const index = virtualRow.index * amountOfColumns + i;
						const item = data[index];

						if (!item) return null;
						return (
							<div
								key={virtualColumn.index}
								style={{
									position: 'absolute',
									top: 0,
									left: 0,
									width: `${virtualColumn.size}px`,
									height: `${virtualRow.size}px`,
									transform: `translateX(${virtualColumn.start}px) translateY(${virtualRow.start}px)`
								}}
							>
								<MediaViewItem key={item.item.id} data={item} index={index} />
							</div>
						);
					})}
				</React.Fragment>
			))}
		</div>
	);
};
