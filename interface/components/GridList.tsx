import { useVirtualizer } from '@tanstack/react-virtual';
import React, {
	ReactNode,
	RefObject,
	useCallback,
	useEffect,
	useLayoutEffect,
	useMemo,
	useRef,
	useState
} from 'react';
import { useMutationObserver } from 'rooks';
import useResizeObserver from 'use-resize-observer';

type ItemData = any | undefined;
type ItemId = number | string;

export interface GridListItem<IdT extends ItemId = number, DataT extends ItemData = undefined> {
	index: number;
	id: IdT;
	row: number;
	column: number;
	rect: Omit<DOMRect, 'toJSON'>;
	data: DataT;
}

export interface UseGridListProps<IdT extends ItemId = number, DataT extends ItemData = undefined> {
	count: number;
	totalCount?: number;
	ref: RefObject<HTMLElement>;
	padding?: number | { x?: number; y?: number };
	gap?: number | { x?: number; y?: number };
	overscan?: number;
	top?: number;
	rowsBeforeLoadMore?: number;
	onLoadMore?: () => void;
	getItemId?: (index: number) => IdT | undefined;
	getItemData?: (index: number) => DataT;
	size?: number | { width: number; height: number };
	columns?: number;
}

export const useGridList = <IdT extends ItemId = number, DataT extends ItemData = undefined>({
	padding,
	gap,
	size,
	columns,
	ref,
	getItemId,
	getItemData,
	...props
}: UseGridListProps<IdT, DataT>) => {
	const { width } = useResizeObserver({ ref });

	const count = props.totalCount ?? props.count;

	const paddingX = (typeof padding === 'object' ? padding.x : padding) || 0;
	const paddingY = (typeof padding === 'object' ? padding.y : padding) || 0;

	const gapX = (typeof gap === 'object' ? gap.x : gap) || 0;
	const gapY = (typeof gap === 'object' ? gap.y : gap) || 0;

	const itemWidth = size ? (typeof size === 'object' ? size.width : size) : undefined;
	const itemHeight = size ? (typeof size === 'object' ? size.height : size) : undefined;

	const gridWidth = width ? width - (paddingX || 0) * 2 : 0;

	let columnCount = columns || 0;

	if (!columns && itemWidth) {
		let columns = Math.floor(gridWidth / itemWidth);
		if (gapX) columns = Math.floor((gridWidth - (columns - 1) * gapX) / itemWidth);
		columnCount = columns;
	}

	const rowCount = columnCount > 0 ? Math.ceil(props.count / columnCount) : 0;
	const totalRowCount = columnCount > 0 ? Math.ceil(count / columnCount) : 0;

	const virtualItemWidth =
		columnCount > 0 ? (gridWidth - (columnCount - 1) * gapX) / columnCount : 0;

	const virtualItemHeight = itemHeight || virtualItemWidth;

	const getItem = useCallback(
		(index: number) => {
			if (index < 0 || index >= count) return;

			const id = getItemId?.(index) || index;

			const data = getItemData?.(index) as DataT;

			const column = index % columnCount;
			const row = Math.floor(index / columnCount);

			const x = paddingX + (column !== 0 ? gapX : 0) * column + virtualItemWidth * column;
			const y = paddingY + (row !== 0 ? gapY : 0) * row + virtualItemHeight * row;

			const item: GridListItem<typeof id, DataT> = {
				index,
				id,
				data,
				row,
				column,
				rect: {
					height: virtualItemHeight,
					width: virtualItemWidth,
					x,
					y,
					top: y,
					bottom: y + virtualItemHeight,
					left: x,
					right: x + virtualItemWidth
				}
			};

			return item;
		},
		[
			columnCount,
			count,
			gapX,
			gapY,
			getItemId,
			getItemData,
			paddingX,
			paddingY,
			virtualItemHeight,
			virtualItemWidth
		]
	);

	return {
		columnCount,
		rowCount,
		totalRowCount,
		width: gridWidth,
		padding: { x: paddingX, y: paddingY },
		gap: { x: gapX, y: gapY },
		itemHeight,
		itemWidth,
		virtualItemHeight,
		virtualItemWidth,
		getItem,
		...props
	};
};

export interface GridListProps {
	grid: ReturnType<typeof useGridList>;
	scrollRef: RefObject<HTMLElement>;
	children: (index: number) => ReactNode;
}

export const GridList = ({ grid, children, scrollRef }: GridListProps) => {
	const ref = useRef<HTMLDivElement>(null);

	const [listOffset, setListOffset] = useState(0);

	const getHeight = useCallback(
		(index: number) => grid.virtualItemHeight + (index !== 0 ? grid.gap.y : 0),
		[grid.virtualItemHeight, grid.gap.y]
	);

	const getWidth = useCallback(
		(index: number) => grid.virtualItemWidth + (index !== 0 ? grid.gap.x : 0),
		[grid.virtualItemWidth, grid.gap.x]
	);

	const rowVirtualizer = useVirtualizer({
		count: grid.totalRowCount,
		getScrollElement: () => scrollRef.current,
		estimateSize: getHeight,
		paddingStart: grid.padding.y,
		paddingEnd: grid.padding.y,
		overscan: grid.overscan,
		scrollMargin: listOffset
	});

	const columnVirtualizer = useVirtualizer({
		horizontal: true,
		count: grid.columnCount,
		getScrollElement: () => scrollRef.current,
		estimateSize: getWidth,
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

		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [rowVirtualizer, columnVirtualizer, grid.columnCount, grid.rowCount]);

	useEffect(() => {
		if (!grid.onLoadMore) return;

		const lastRow = virtualRows[virtualRows.length - 1];
		if (!lastRow) return;

		const rowsBeforeLoadMore = grid.rowsBeforeLoadMore || 1;

		const loadMoreOnIndex =
			rowsBeforeLoadMore > grid.rowCount || lastRow.index > grid.rowCount - rowsBeforeLoadMore
				? grid.rowCount - 1
				: grid.rowCount - rowsBeforeLoadMore;

		if (lastRow.index === loadMoreOnIndex || lastRow.index > grid.rowCount) grid.onLoadMore();
	}, [virtualRows, grid.rowCount, grid.rowsBeforeLoadMore, grid.onLoadMore, grid]);

	useMutationObserver(scrollRef, () => setListOffset(ref.current?.offsetTop ?? 0));

	useLayoutEffect(() => setListOffset(ref.current?.offsetTop ?? 0), []);

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
							const index = virtualRow.index * grid.columnCount + virtualColumn.index;

							if (index >= grid.count) return null;

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
										paddingLeft: virtualColumn.index !== 0 ? grid.gap.x : 0,
										paddingTop: virtualRow.index !== 0 ? grid.gap.y : 0
									}}
								>
									<div
										className="m-auto"
										style={{
											width: grid.itemWidth || '100%',
											height: grid.itemHeight || '100%'
										}}
									>
										{children(index)}
									</div>
								</div>
							);
						})}
					</React.Fragment>
				))}
		</div>
	);
};
