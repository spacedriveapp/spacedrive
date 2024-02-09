import { LoadMoreTrigger, useGrid, useScrollMargin, useVirtualizer } from '@virtual-grid/react';
import React, { useCallback, useEffect, useMemo, useRef } from 'react';
import { getItemFilePath } from '@sd/client';

import { useExplorerContext } from '../../Context';
import { orderingKey } from '../../store';
import { getItemData, getItemId, uniqueId } from '../../util';
import { useExplorerViewContext } from '../Context';
import { DragSelect } from '../Grid/DragSelect';
import { GridItem } from '../Grid/Item';
import { useKeySelection } from '../Grid/useKeySelection';
import { DATE_HEADER_HEIGHT, DateHeader } from './DateHeader';
import { MediaViewItem } from './Item';
import { formatDate } from './util';

export const MediaView = () => {
	const explorer = useExplorerContext();
	const explorerView = useExplorerViewContext();
	const explorerSettings = explorer.useSettingsSnapshot();

	const gridRef = useRef<HTMLDivElement>(null);

	const isSortingByDate = explorerSettings.order
		? orderingKey(explorerSettings.order).toLowerCase().includes('date')
		: undefined;

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
		if (!isSortingByDate) return;

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

		// Get the index of the last item and exclude any total count indexes
		let lastItemIndex = lastRowIndex * grid.columnCount + grid.columnCount;
		if (lastItemIndex > grid.options.count - 1) lastItemIndex = grid.options.count - 1;

		const firstExplorerItem = explorer.items?.[firstRowIndex * grid.columnCount];
		const lastExplorerItem = explorer.items?.[lastItemIndex];

		const firstFilePath = firstExplorerItem && getItemFilePath(firstExplorerItem);
		if (!firstFilePath) return;

		const lastFilePath = lastExplorerItem && getItemFilePath(lastExplorerItem);
		if (!lastFilePath) return;

		const firstDateCreated = firstFilePath.date_created
			? new Date(new Date(firstFilePath.date_created).setHours(0, 0, 0, 0))
			: undefined;

		const lastDateCreated = lastFilePath.date_created
			? new Date(new Date(lastFilePath.date_created).setHours(0, 0, 0, 0))
			: undefined;

		if (!firstDateCreated || !lastDateCreated) return;

		if (firstDateCreated.getTime() !== lastDateCreated.getTime()) {
			return formatDate({ from: firstDateCreated, to: lastDateCreated });
		}

		return formatDate(firstDateCreated);
	}, [
		explorer.items,
		grid.columnCount,
		grid.options.count,
		isSortingByDate,
		rowVirtualizer.scrollOffset,
		rowVirtualizer.scrollRect.height,
		virtualRows
	]);

	const { activeItem } = useKeySelection(grid);

	return (
		<div
			ref={gridRef}
			style={{
				position: 'relative',
				height: `${rowVirtualizer.getTotalSize()}px`,
				width: '100%'
			}}
		>
			{isSortingByDate && <DateHeader date={date ?? ''} />}

			<DragSelect grid={grid} onActiveItemChange={(item) => (activeItem.current = item)}>
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
