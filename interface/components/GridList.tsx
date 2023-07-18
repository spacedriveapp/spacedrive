import { useVirtualizer } from '@tanstack/react-virtual';
import React, { useCallback, useRef } from 'react';
import { RefObject, useEffect, useMemo, useState } from 'react';
import { useBoundingclientrect } from 'rooks';
import useResizeObserver from 'use-resize-observer';

type UseGridItemData = Record<any, any>;
type Id = string | number;
interface UseGridItem<DataT extends UseGridItemData, IdT extends Id> {
	id: IdT;
	index: number;
	rect: {
		height: number;
		width: number;
		top: number;
		bottom: number;
		left: number;
		right: number;
	};
	row: number;
	data?: DataT;
}
type UseGridItems<DataT extends UseGridItemData, IdT extends Id> = {
	items: UseGridItem<DataT, IdT>[];
	itemsById: Record<string, UseGridItem<DataT, IdT>>;
};
interface UseGridPropsDefaults<DataT extends UseGridItemData, IdT extends Id> {
	count: number;
	getItemData?: (index: number) => DataT | undefined;
	ref: RefObject<HTMLElement>;
	padding?: number | { x?: number; y?: number };
	gap?: number | { x?: number; y?: number };
	overscan?: number;
	top?: number;
	rowsBeforeLoadMore?: number;
	onLoadMore?: () => void;
	getItemId?: (item: DataT, index: number) => IdT;
	size?: number | { width: number; height: number };
	columns?: number;
}

export const useGridList = <DataT extends UseGridItemData, IdT extends Id>({
	count,
	padding,
	gap,
	size,
	columns,
	getItemId,
	getItemData,
	ref,
	...props
}: UseGridPropsDefaults<DataT, IdT>) => {
	const { width } = useResizeObserver({ ref });

	const paddingX = (typeof padding === 'object' ? padding.x : padding) || 0;
	const paddingY = (typeof padding === 'object' ? padding.y : padding) || 0;

	const gapX = (typeof gap === 'object' ? gap.x : gap) || 0;
	const gapY = (typeof gap === 'object' ? gap.y : gap) || 0;

	const itemWidth = size ? (typeof size === 'object' ? size.width : size) : undefined;

	const itemHeight = size ? (typeof size === 'object' ? size.height : size) : undefined;

	const gridWidth = width ? width - (paddingX || 0) * 2 : 0;

	// Virtualizer count calculation
	const amountOfColumns = columns ? columns : itemWidth ? Math.floor(gridWidth / itemWidth) : 0;
	const amountOfRows = amountOfColumns > 0 ? Math.ceil(count / amountOfColumns) : 0;

	// Virtualizer item size calculation
	const virtualItemWidth = amountOfColumns > 0 ? gridWidth / amountOfColumns : 0;
	const virtualItemHeight = itemHeight || virtualItemWidth;

	const getGridItems = useCallback(() => {
		if (width === 0) return {} as UseGridItems<DataT, IdT>;

		const items = Array.from<undefined>({ length: count }).reduce(
			(items, _, i) => {
				const column = i % amountOfColumns;
				const row = Math.floor(i / amountOfColumns);

				const x = paddingX + gapX * column + virtualItemWidth * column;
				const y = paddingY + gapY * row + virtualItemHeight * row;

				const bottom = y + virtualItemHeight;

				const data = getItemData?.(i);
				const id = (data && getItemId?.(data, i)) || i;

				const item: UseGridItem<DataT, IdT> = {
					id: id as IdT,
					data: data,
					index: i,
					row: row,
					rect: {
						width: virtualItemWidth,
						height: virtualItemHeight,
						top: y,
						bottom: bottom,
						left: x,
						right: x + virtualItemWidth
					}
				};

				return {
					items: [...items.items, item],
					itemsById: { ...items.itemsById, [id]: item }
				} satisfies UseGridItems<DataT, IdT>;
			},
			{ items: [], itemsById: {} } as UseGridItems<DataT, IdT>
		);

		return items;
	}, [
		width,
		count,
		amountOfColumns,
		paddingX,
		gapX,
		virtualItemWidth,
		paddingY,
		gapY,
		virtualItemHeight,
		getItemData,
		getItemId
	]);

	return {
		getGridItems,
		padding: { x: paddingX, y: paddingY },
		gap: { x: gapX, y: gapY },
		amountOfColumns,
		amountOfRows,
		width: gridWidth,
		virtualItemWidth,
		virtualItemHeight,
		itemHeight,
		itemWidth,
		...props
	};
};

export interface GridListProps {
	grid: ReturnType<typeof useGridList>;
	children: (index: number) => JSX.Element | null;
	scrollRef: RefObject<HTMLElement>;
}

export const GridList = ({ grid, children, scrollRef }: GridListProps) => {
	const ref = useRef<HTMLDivElement>(null);

	const rect = useBoundingclientrect(ref);

	const [listOffset, setListOffset] = useState(0);

	const rowVirtualizer = useVirtualizer({
		count: grid.amountOfRows,
		getScrollElement: () => scrollRef.current,
		estimateSize: () => grid.virtualItemHeight,
		measureElement: () => grid.virtualItemHeight,
		paddingStart: grid.padding.y,
		paddingEnd: grid.padding.y,
		overscan: grid.overscan,
		scrollMargin: listOffset
	});

	const columnVirtualizer = useVirtualizer({
		horizontal: true,
		count: grid.amountOfColumns,
		getScrollElement: () => scrollRef.current,
		estimateSize: () => grid.virtualItemWidth,
		measureElement: () => grid.virtualItemWidth,
		paddingStart: grid.padding.x,
		paddingEnd: grid.padding.x
	});

	const virtualRows = rowVirtualizer.getVirtualItems();
	const virtualColumns = columnVirtualizer.getVirtualItems();

	// Measure virtual item on size change
	useEffect(() => {
		rowVirtualizer.measure();
		columnVirtualizer.measure();
	}, [rowVirtualizer, columnVirtualizer, grid.virtualItemWidth, grid.virtualItemHeight]);

	// Force recalculate range
	// https://github.com/TanStack/virtual/issues/485
	useMemo(() => {
		// @ts-ignore
		rowVirtualizer.calculateRange();
		// @ts-ignore
		columnVirtualizer.calculateRange();
	}, [rowVirtualizer, columnVirtualizer, grid.amountOfColumns, grid.amountOfRows]);

	// TODO: Improve this
	useEffect(() => {
		setListOffset(ref.current?.offsetTop || 0);
	}, [rect]);

	useEffect(() => {
		if (grid.onLoadMore) {
			const lastRow = virtualRows[virtualRows.length - 1];
			if (lastRow) {
				const rowsBeforeLoadMore = grid.rowsBeforeLoadMore || 1;

				const loadMoreOnIndex =
					rowsBeforeLoadMore > grid.amountOfRows ||
					lastRow.index > grid.amountOfRows - rowsBeforeLoadMore
						? grid.amountOfRows - 1
						: grid.amountOfRows - rowsBeforeLoadMore;

				if (lastRow.index === loadMoreOnIndex) grid.onLoadMore();
			}
		}
	}, [virtualRows, grid.amountOfRows, grid.rowsBeforeLoadMore, grid.onLoadMore, grid]);

	return (
		<div
			ref={ref}
			className="relative w-full overflow-x-hidden"
			style={{
				height: `${rowVirtualizer.getTotalSize()}px`
			}}
		>
			{grid.width > 0 &&
				virtualRows.map((virtualRow) => (
					<React.Fragment key={virtualRow.index}>
						{virtualColumns.map((virtualColumn) => {
							const index =
								virtualRow.index * grid.amountOfColumns + virtualColumn.index;

							const item = children(index);
							if (!item) return null;

							const padding =
								(virtualColumn.size - (grid.itemWidth || grid.virtualItemWidth)) /
								2;

							return (
								<div
									key={virtualColumn.index}
									style={{
										position: 'absolute',
										top: 0,
										left: 0,
										width: `${virtualColumn.size}px`,
										height: `${virtualRow.size}px`,
										transform: `translateX(${
											virtualColumn.start
										}px) translateY(${
											virtualRow.start - rowVirtualizer.options.scrollMargin
										}px)`,
										paddingLeft: padding,
										paddingRight: padding
									}}
								>
									{item}
								</div>
							);
						})}
					</React.Fragment>
				))}
		</div>
	);
};
