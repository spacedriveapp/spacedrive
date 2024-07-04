import { LoadMoreTrigger, useGrid, useScrollMargin, useVirtualizer } from '@virtual-grid/react';
import React, { useCallback, useEffect, useMemo, useRef } from 'react';
import { getOrderingDirection, getOrderingKey, OrderingKey } from '@sd/client';
import { useLocale } from '~/hooks';

import { useExplorerContext } from '../../Context';
import { getItemData, getItemId, uniqueId } from '../../util';
import { useExplorerViewContext } from '../Context';
import { DragSelect } from '../Grid/DragSelect';
import { GridItem } from '../Grid/Item';
import { useKeySelection } from '../Grid/useKeySelection';
import { DATE_HEADER_HEIGHT, DateHeader } from './DateHeader';
import { MediaViewItem } from './Item';
import { formatDate, getDate } from './util';

const SORT_BY_DATE: Partial<Record<OrderingKey, boolean>> = {
	'dateCreated': true,
	'dateIndexed': true,
	'dateModified': true,
	'object.dateAccessed': true,
	'object.mediaData.epochTime': true
};

export const MediaView = () => {
	const explorer = useExplorerContext();
	const explorerView = useExplorerViewContext();
	const explorerSettings = explorer.useSettingsSnapshot();

	const gridRef = useRef<HTMLDivElement>(null);

	const orderBy = explorerSettings.order && getOrderingKey(explorerSettings.order);
	const orderDirection = explorerSettings.order && getOrderingDirection(explorerSettings.order);

	const { dateFormat } = useLocale();

	const isSortingByDate = orderBy && SORT_BY_DATE[orderBy];

	const grid = useGrid({
		scrollRef: explorer.scrollRef,
		count: explorer.items?.length ?? 0,
		totalCount: explorer.count,
		columns: explorerSettings.mediaColumns,
		padding: {
			top: isSortingByDate ? DATE_HEADER_HEIGHT : 0,
			bottom: explorerView.scrollPadding?.bottom
		},
		gap: 1,
		overscan: explorer.overscan ?? 5,
		onLoadMore: explorer.loadMore,
		getItemId: useCallback(
			(index: number) => getItemId(index, explorer.items ?? []),
			[explorer.items]
		),
		getItemData: useCallback(
			(index: number) => getItemData(index, explorer.items ?? []),
			[explorer.items]
		)
	});

	const { scrollMargin } = useScrollMargin({ scrollRef: explorer.scrollRef, gridRef });

	const rowVirtualizer = useVirtualizer({
		...grid.rowVirtualizer,
		scrollMargin: scrollMargin.top
	});

	const columnVirtualizer = useVirtualizer(grid.columnVirtualizer);

	useEffect(() => {
		rowVirtualizer.measure();
		columnVirtualizer.measure();
	}, [rowVirtualizer, columnVirtualizer, grid.virtualItemHeight]);

	const virtualRows = rowVirtualizer.getVirtualItems();

	const date = useMemo(() => {
		if (!isSortingByDate || !orderBy || !orderDirection) return;

		// Prevent date placeholder from showing when
		// items are still fetching
		if (explorer.items === null) return '';

		let firstRowIndex: number | undefined = undefined;
		let lastRowIndex: number | undefined = undefined;

		// Find first row in viewport
		for (let i = 0; i < virtualRows.length; i++) {
			const row = virtualRows[i]!;
			if (row.end >= rowVirtualizer.scrollOffset) {
				firstRowIndex = row.index;
				break;
			}
		}

		// Find last row in viewport
		for (let i = virtualRows.length - 1; i >= 0; i--) {
			const row = virtualRows[i]!;
			if (row.start <= rowVirtualizer.scrollOffset + rowVirtualizer.scrollRect.height) {
				lastRowIndex = row.index;
				break;
			}
		}

		if (firstRowIndex === undefined || lastRowIndex === undefined) return;

		let firstItemIndex = firstRowIndex * grid.columnCount;
		let lastItemIndex = lastRowIndex * grid.columnCount + grid.columnCount;

		// Exclude any total count indexes
		if (lastItemIndex > grid.options.count - 1) lastItemIndex = grid.options.count - 1;

		let firstFilePathDate: string | null = null;
		let lastFilePathDate: string | null = null;

		// Look for the first date
		for (let i = firstItemIndex; i < lastItemIndex; i++) {
			const item = explorer.items[i];
			const date = item && getDate(item, orderBy);

			if (!date) {
				if (i !== lastItemIndex - 1) firstItemIndex++;
				continue;
			}

			firstFilePathDate = date;
			break;
		}

		// Look for the last date up to where the first lookup ended
		for (let i = lastItemIndex; i > firstItemIndex; i--) {
			const item = explorer.items[i];
			const date = item && getDate(item, orderBy);

			if (!date) continue;

			lastFilePathDate = date;
			break;
		}

		const firstDate = firstFilePathDate
			? new Date(new Date(firstFilePathDate).setHours(0, 0, 0, 0))
			: undefined;

		const lastDate = lastFilePathDate
			? new Date(new Date(lastFilePathDate).setHours(0, 0, 0, 0))
			: undefined;

		if (firstDate && !lastDate) return formatDate(firstDate, dateFormat);

		if (!firstDate && lastDate) return formatDate(lastDate, dateFormat);

		if (firstDate && lastDate) {
			if (firstDate.getTime() === lastDate.getTime()) {
				return formatDate(firstDate, dateFormat);
			}

			return formatDate(
				{
					from: orderDirection === 'Asc' ? firstDate : lastDate,
					to: orderDirection === 'Asc' ? lastDate : firstDate
				},
				dateFormat
			);
		}
	}, [
		explorer.items,
		grid.columnCount,
		grid.options.count,
		isSortingByDate,
		rowVirtualizer.scrollOffset,
		rowVirtualizer.scrollRect.height,
		virtualRows,
		orderBy,
		orderDirection
	]);

	useKeySelection(grid);

	return (
		<div
			ref={gridRef}
			style={{
				position: 'relative',
				height: `${rowVirtualizer.getTotalSize()}px`,
				width: '100%'
			}}
		>
			{isSortingByDate && <DateHeader date={date} />}

			<DragSelect grid={grid}>
				{virtualRows.map((virtualRow) => (
					<React.Fragment key={virtualRow.key}>
						{columnVirtualizer.getVirtualItems().map((virtualColumn) => {
							const virtualItem = grid.getVirtualItem({
								row: virtualRow,
								column: virtualColumn,
								scrollMargin
							});

							const item = virtualItem && explorer.items?.[virtualItem.index];
							if (!item) return null;

							return (
								<div key={uniqueId(item)} style={virtualItem.style}>
									<GridItem index={virtualItem.index} item={item}>
										{({ selected, cut }) => (
											<MediaViewItem
												data={item}
												selected={selected}
												cover={explorerSettings.mediaAspectSquare}
												cut={cut}
											/>
										)}
									</GridItem>
								</div>
							);
						})}
					</React.Fragment>
				))}
			</DragSelect>

			<LoadMoreTrigger {...grid.getLoadMoreTrigger({ virtualizer: rowVirtualizer })} />
		</div>
	);
};
