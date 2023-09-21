import { CaretDown, CaretUp } from '@phosphor-icons/react';
import {
	flexRender,
	VisibilityState,
	type ColumnSizingState,
	type Row
} from '@tanstack/react-table';
import { useVirtualizer } from '@tanstack/react-virtual';
import clsx from 'clsx';
import { memo, useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import BasicSticky from 'react-sticky-el';
import { useKey, useMutationObserver, useWindowEventListener } from 'rooks';
import useResizeObserver from 'use-resize-observer';
import { type ExplorerItem } from '@sd/client';
import { ContextMenu, Tooltip } from '@sd/ui';
import { useIsTextTruncated } from '~/hooks';
import { isNonEmptyObject } from '~/util';

import { useLayoutContext } from '../../../Layout/Context';
import { useExplorerContext } from '../../Context';
import { getQuickPreviewStore } from '../../QuickPreview/store';
import {
	createOrdering,
	getOrderingDirection,
	isCut,
	orderingKey,
	useExplorerStore
} from '../../store';
import { uniqueId } from '../../util';
import { useExplorerViewContext } from '../../ViewContext';
import { ViewItem } from '../ViewItem';
import { getRangeDirection, Range, useRanges } from './util/ranges';
import { useTable } from './util/table';

interface ListViewItemProps {
	row: Row<ExplorerItem>;
	paddingX: number;
	// Props below are passed to trigger a rerender
	// should probably use a better solution
	columnSizing: ColumnSizingState;
	columnVisibility: VisibilityState;
	isCut: boolean;
	isRenaming: boolean;
}

const ListViewItem = memo((props: ListViewItemProps) => {
	return (
		<ViewItem
			data={props.row.original}
			className="relative flex h-full items-center"
			style={{ paddingLeft: props.paddingX, paddingRight: props.paddingX }}
		>
			{props.row.getVisibleCells().map((cell) => (
				<div
					role="cell"
					key={cell.id}
					className={clsx(
						'table-cell shrink-0 truncate px-4 text-xs text-ink-dull',
						cell.column.columnDef.meta?.className
					)}
					style={{ width: cell.column.getSize() }}
				>
					{flexRender(cell.column.columnDef.cell, cell.getContext())}
				</div>
			))}
		</ViewItem>
	);
});

const HeaderColumnName = ({ name }: { name: string }) => {
	const textRef = useRef<HTMLParagraphElement>(null);

	const isTruncated = useIsTextTruncated(textRef, name);

	return (
		<div ref={textRef} className="truncate">
			{isTruncated ? (
				<Tooltip label={name}>
					<span className="truncate">{name}</span>
				</Tooltip>
			) : (
				<span>{name}</span>
			)}
		</div>
	);
};

const ROW_HEIGHT = 45;

export default () => {
	const layout = useLayoutContext();
	const explorer = useExplorerContext();
	const explorerStore = useExplorerStore();
	const { isRenaming, ...explorerView } = useExplorerViewContext();
	const settings = explorer.useSettingsSnapshot();

	const tableRef = useRef<HTMLDivElement>(null);
	const tableHeaderRef = useRef<HTMLDivElement>(null);
	const tableBodyRef = useRef<HTMLDivElement>(null);

	const scrollLeft = useRef(0);

	const [sized, setSized] = useState(false);
	const [locked, setLocked] = useState(false);
	const [resizing, setResizing] = useState(false);
	const [top, setTop] = useState(0);
	const [listOffset, setListOffset] = useState(0);
	const [ranges, setRanges] = useState<Range[]>([]);

	const [isLeftMouseDown, setIsLeftMouseDown] = useState(false);
	const [dragging, setDragging] = useState(false);

	const { table } = useTable();
	const { columnVisibility, columnSizing } = table.getState();
	const { rows, rowsById } = table.getRowModel();

	const { getRangeByIndex, getRangesByRow, getClosestRange } = useRanges({
		ranges,
		rows: rowsById
	});

	const padding = {
		x: explorerView.padding?.x ?? 16,
		y: explorerView.padding?.y ?? 12
	};

	const rowVirtualizer = useVirtualizer({
		count: explorer.count ?? rows.length,
		getScrollElement: useCallback(() => explorer.scrollRef.current, [explorer.scrollRef]),
		estimateSize: useCallback(() => ROW_HEIGHT, []),
		paddingStart: padding.y,
		paddingEnd: padding.y,
		scrollMargin: listOffset,
		overscan: explorer.overscan ?? 10
	});

	const virtualRows = rowVirtualizer.getVirtualItems();

	function handleRowClick(
		e: React.MouseEvent<HTMLDivElement, MouseEvent>,
		row: Row<ExplorerItem>
	) {
		// Ensure mouse click is with left button
		if (e.button !== 0) return;

		const rowIndex = row.index;
		const item = row.original;

		if (explorer.allowMultiSelect) {
			if (e.shiftKey) {
				const range = getRangeByIndex(ranges.length - 1);

				if (!range) {
					const items = [...Array(rowIndex + 1)].reduce<ExplorerItem[]>((items, _, i) => {
						const item = rows[i]?.original;
						if (item) return [...items, item];
						return items;
					}, []);

					const [rangeStart] = items;

					if (rangeStart) {
						setRanges([[uniqueId(rangeStart), uniqueId(item)]]);
					}

					explorer.resetSelectedItems(items);
					return;
				}

				const direction = getRangeDirection(range.end.index, rowIndex);

				if (!direction) return;

				const changeDirection =
					!!range.direction &&
					range.direction !== direction &&
					(direction === 'down'
						? rowIndex > range.start.index
						: rowIndex < range.start.index);

				let _ranges = ranges;

				const [backRange, frontRange] = getRangesByRow(range.start);

				if (backRange && frontRange) {
					[
						...Array(backRange.sorted.end.index - backRange.sorted.start.index + 1)
					].forEach((_, i) => {
						const index = backRange.sorted.start.index + i;

						if (index === range.start.index) return;

						const row = rows[index];

						if (row) explorer.removeSelectedItem(row.original);
					});

					_ranges = _ranges.filter((_, i) => i !== backRange.index);
				}

				[
					...Array(Math.abs(range.end.index - rowIndex) + (changeDirection ? 1 : 0))
				].forEach((_, i) => {
					if (!range.direction || direction === range.direction) i += 1;

					const index = range.end.index + (direction === 'down' ? i : -i);

					const row = rows[index];

					if (!row) return;

					const item = row.original;

					if (uniqueId(item) === uniqueId(range.start.original)) return;

					if (
						!range.direction ||
						direction === range.direction ||
						(changeDirection &&
							(range.direction === 'down'
								? index < range.start.index
								: index > range.start.index))
					) {
						explorer.addSelectedItem(item);
					} else explorer.removeSelectedItem(item);
				});

				let newRangeEnd = item;
				let removeRangeIndex: number | null = null;

				for (let i = 0; i < _ranges.length - 1; i++) {
					const range = getRangeByIndex(i);

					if (!range) continue;

					if (
						rowIndex >= range.sorted.start.index &&
						rowIndex <= range.sorted.end.index
					) {
						const removableRowsCount = Math.abs(
							(direction === 'down'
								? range.sorted.end.index
								: range.sorted.start.index) - rowIndex
						);

						[...Array(removableRowsCount)].forEach((_, i) => {
							i += 1;

							const index = rowIndex + (direction === 'down' ? i : -i);

							const row = rows[index];

							if (row) explorer.removeSelectedItem(row.original);
						});

						removeRangeIndex = i;
						break;
					} else if (direction === 'down' && rowIndex + 1 === range.sorted.start.index) {
						newRangeEnd = range.sorted.end.original;
						removeRangeIndex = i;
						break;
					} else if (direction === 'up' && rowIndex - 1 === range.sorted.end.index) {
						newRangeEnd = range.sorted.start.original;
						removeRangeIndex = i;
						break;
					}
				}

				if (removeRangeIndex !== null) {
					_ranges = _ranges.filter((_, i) => i !== removeRangeIndex);
				}

				setRanges([
					..._ranges.slice(0, _ranges.length - 1),
					[uniqueId(range.start.original), uniqueId(newRangeEnd)]
				]);
			} else if (e.metaKey) {
				if (explorer.selectedItems.has(item)) {
					explorer.removeSelectedItem(item);

					const rowRanges = getRangesByRow(row);

					const range = rowRanges[0] || rowRanges[1];

					if (range) {
						const rangeStart = range.sorted.start.original;
						const rangeEnd = range.sorted.end.original;

						if (rangeStart === rangeEnd) {
							const closestRange = getClosestRange(range.index);
							if (closestRange) {
								const _ranges = ranges.filter(
									(_, i) => i !== closestRange.index && i !== range.index
								);

								const start = closestRange.sorted.start.original;
								const end = closestRange.sorted.end.original;

								setRanges([
									..._ranges,
									[
										uniqueId(closestRange.direction === 'down' ? start : end),
										uniqueId(closestRange.direction === 'down' ? end : start)
									]
								]);
							} else {
								setRanges([]);
							}
						} else if (rangeStart === item || rangeEnd === item) {
							const _ranges = ranges.filter(
								(_, i) => i !== range.index && i !== rowRanges[1]?.index
							);

							const start =
								rows[
									rangeStart === item
										? range.sorted.start.index + 1
										: range.sorted.end.index - 1
								]?.original;

							if (start !== undefined) {
								const end = rangeStart === item ? rangeEnd : rangeStart;

								setRanges([..._ranges, [uniqueId(start), uniqueId(end)]]);
							}
						} else {
							const rowBefore = rows[row.index - 1];
							const rowAfter = rows[row.index + 1];

							if (rowBefore && rowAfter) {
								const firstRange = [
									uniqueId(rangeStart),
									uniqueId(rowBefore.original)
								] satisfies Range;

								const secondRange = [
									uniqueId(rowAfter.original),
									uniqueId(rangeEnd)
								] satisfies Range;

								const _ranges = ranges.filter(
									(_, i) => i !== range.index && i !== rowRanges[1]?.index
								);

								setRanges([..._ranges, firstRange, secondRange]);
							}
						}
					}
				} else {
					explorer.addSelectedItem(item);

					const itemRange: Range = [uniqueId(item), uniqueId(item)];

					const _ranges = [...ranges, itemRange];

					const rangeDown = getClosestRange(_ranges.length - 1, {
						direction: 'down',
						maxRowDifference: 0,
						ranges: _ranges
					});

					const rangeUp = getClosestRange(_ranges.length - 1, {
						direction: 'up',
						maxRowDifference: 0,
						ranges: _ranges
					});

					if (rangeDown && rangeUp) {
						const _ranges = ranges.filter(
							(_, i) => i !== rangeDown.index && i !== rangeUp.index
						);

						setRanges([
							..._ranges,
							[
								uniqueId(rangeUp.sorted.start.original),
								uniqueId(rangeDown.sorted.end.original)
							],
							itemRange
						]);
					} else if (rangeUp || rangeDown) {
						const closestRange = rangeDown || rangeUp;

						if (closestRange) {
							const _ranges = ranges.filter((_, i) => i !== closestRange.index);

							setRanges([
								..._ranges,
								[
									uniqueId(item),
									uniqueId(
										closestRange.direction === 'down'
											? closestRange.sorted.end.original
											: closestRange.sorted.start.original
									)
								]
							]);
						}
					} else {
						setRanges([...ranges, itemRange]);
					}
				}
			} else {
				if (explorer.isItemSelected(item)) return;

				explorer.resetSelectedItems([item]);
				const hash = uniqueId(item);
				setRanges([[hash, hash]]);
			}
		} else {
			explorer.resetSelectedItems([item]);
		}
	}

	function handleRowContextMenu(row: Row<ExplorerItem>) {
		if (explorerView.contextMenu === undefined) return;

		const item = row.original;

		if (!explorer.isItemSelected(item)) {
			explorer.resetSelectedItems([item]);
			const hash = uniqueId(item);
			setRanges([[hash, hash]]);
		}
	}

	// Reset ranges
	useEffect(() => setRanges([]), [explorer.items]);

	// Measure initial column widths
	useEffect(() => {
		if (
			!tableRef.current ||
			sized ||
			!isNonEmptyObject(columnSizing) ||
			!isNonEmptyObject(columnVisibility)
		) {
			return;
		}

		const sizing = table
			.getVisibleLeafColumns()
			.reduce(
				(sizing, column) => ({ ...sizing, [column.id]: column.getSize() }),
				{} as ColumnSizingState
			);

		const tableWidth = tableRef.current.offsetWidth;
		const columnsWidth = Object.values(sizing).reduce((a, b) => a + b, 0) + padding.x * 2;

		if (columnsWidth < tableWidth) {
			const nameWidth = (sizing.name ?? 0) + (tableWidth - columnsWidth);
			table.setColumnSizing({ ...sizing, name: nameWidth });
			setLocked(true);
		} else if (columnsWidth > tableWidth) {
			const nameColSize = sizing.name ?? 0;
			const minNameColSize = table.getColumn('name')?.columnDef.minSize;

			const difference = columnsWidth - tableWidth;

			if (minNameColSize !== undefined && nameColSize - difference >= minNameColSize) {
				table.setColumnSizing({ ...sizing, name: nameColSize - difference });
				setLocked(true);
			}
		} else if (columnsWidth === tableWidth) {
			setLocked(true);
		}

		setSized(true);
	}, [columnSizing, columnVisibility, padding.x, sized, table]);

	// Load more items
	useEffect(() => {
		if (!explorer.loadMore) return;

		const lastRow = virtualRows[virtualRows.length - 1];
		if (!lastRow) return;

		const loadMoreFromRow = Math.ceil(rows.length * 0.75);

		if (lastRow.index >= loadMoreFromRow - 1) explorer.loadMore.call(undefined);
	}, [virtualRows, rows.length, explorer.loadMore]);

	// Sync scroll
	useEffect(() => {
		const table = tableRef.current;
		const header = tableHeaderRef.current;
		const body = tableBodyRef.current;

		if (!table || !header || !body) return;

		const onScroll = (event: WheelEvent) => {
			event.deltaX !== 0 && event.preventDefault();
			header.scrollLeft += event.deltaX;
			body.scrollLeft += event.deltaX;
		};

		body.addEventListener('scroll', (e) => e.preventDefault());
		table.addEventListener('wheel', onScroll);
		return () => {
			table.removeEventListener('wheel', onScroll);
			body.removeEventListener('scroll', (e) => e.preventDefault());
		};
	}, [sized]);

	// Handle key selection
	useKey(['ArrowUp', 'ArrowDown', 'Escape'], (e) => {
		if (!explorerView.selectable) return;

		e.preventDefault();

		const range = getRangeByIndex(ranges.length - 1);

		if (e.key === 'ArrowDown' && explorer.selectedItems.size === 0) {
			const item = rows[0]?.original;
			if (item) {
				explorer.addSelectedItem(item);
				setRanges([[uniqueId(item), uniqueId(item)]]);
			}
			return;
		}

		if (!range) return;

		if (e.key === 'Escape') {
			explorer.resetSelectedItems([]);
			setRanges([]);
			return;
		}

		const keyDirection = e.key === 'ArrowDown' ? 'down' : 'up';

		const nextRow = rows[range.end.index + (keyDirection === 'up' ? -1 : 1)];

		if (!nextRow) return;

		const item = nextRow.original;

		if (explorer.allowMultiSelect) {
			if (e.shiftKey && !getQuickPreviewStore().open) {
				const direction = range.direction || keyDirection;

				const [backRange, frontRange] = getRangesByRow(range.start);

				if (
					range.direction
						? keyDirection !== range.direction
						: backRange?.direction &&
						  (backRange.sorted.start.index === frontRange?.sorted.start.index ||
								backRange.sorted.end.index === frontRange?.sorted.end.index)
				) {
					explorer.removeSelectedItem(range.end.original);

					if (backRange && frontRange) {
						let _ranges = [...ranges];

						_ranges[backRange.index] = [
							uniqueId(
								backRange.direction !== keyDirection
									? backRange.start.original
									: nextRow.original
							),
							uniqueId(
								backRange.direction !== keyDirection
									? nextRow.original
									: backRange.end.original
							)
						];

						if (
							nextRow.index === backRange.start.index ||
							nextRow.index === backRange.end.index
						) {
							_ranges = _ranges.filter((_, i) => i !== frontRange.index);
						} else {
							_ranges[frontRange.index] =
								frontRange.start.index === frontRange.end.index
									? [uniqueId(nextRow.original), uniqueId(nextRow.original)]
									: [
											uniqueId(frontRange.start.original),
											uniqueId(nextRow.original)
									  ];
						}

						setRanges(_ranges);
					} else {
						setRanges([
							...ranges.slice(0, ranges.length - 1),
							[uniqueId(range.start.original), uniqueId(nextRow.original)]
						]);
					}
				} else {
					explorer.addSelectedItem(item);

					let rangeEndRow = nextRow;

					const closestRange = getClosestRange(range.index, {
						maxRowDifference: 1,
						direction
					});

					if (closestRange) {
						rangeEndRow =
							direction === 'down'
								? closestRange.sorted.end
								: closestRange.sorted.start;
					}

					if (backRange && frontRange) {
						let _ranges = [...ranges];

						const backRangeStart = backRange.start.original;

						const backRangeEnd =
							rangeEndRow.index < backRange.sorted.start.index ||
							rangeEndRow.index > backRange.sorted.end.index
								? rangeEndRow.original
								: backRange.end.original;

						_ranges[backRange.index] = [
							uniqueId(backRangeStart),
							uniqueId(backRangeEnd)
						];

						if (
							backRange.direction !== direction &&
							(rangeEndRow.original === backRangeStart ||
								rangeEndRow.original === backRangeEnd)
						) {
							_ranges[backRange.index] =
								rangeEndRow.original === backRangeStart
									? [uniqueId(backRangeEnd), uniqueId(backRangeStart)]
									: [uniqueId(backRangeStart), uniqueId(backRangeEnd)];
						}

						_ranges[frontRange.index] = [
							uniqueId(frontRange.start.original),
							uniqueId(rangeEndRow.original)
						];

						if (closestRange) {
							_ranges = _ranges.filter((_, i) => i !== closestRange.index);
						}

						setRanges(_ranges);
					} else {
						const _ranges = closestRange
							? ranges.filter((_, i) => i !== closestRange.index && i !== range.index)
							: ranges;

						setRanges([
							..._ranges.slice(0, _ranges.length - 1),
							[uniqueId(range.start.original), uniqueId(rangeEndRow.original)]
						]);
					}
				}
			} else {
				explorer.resetSelectedItems([item]);
				const hash = uniqueId(item);
				setRanges([[hash, hash]]);
			}
		} else explorer.resetSelectedItems([item]);

		if (explorer.scrollRef.current) {
			const tableBodyRect = tableBodyRef.current?.getBoundingClientRect();
			const scrollRect = explorer.scrollRef.current.getBoundingClientRect();

			const paddingTop = parseInt(getComputedStyle(explorer.scrollRef.current).paddingTop);

			const top =
				(explorerView.top ? paddingTop + explorerView.top : paddingTop) +
				scrollRect.top +
				(explorer.scrollRef.current.scrollTop > listOffset ? 36 : 0);

			const rowTop =
				nextRow.index * ROW_HEIGHT +
				rowVirtualizer.options.paddingStart +
				(tableBodyRect?.top || 0) +
				scrollRect.top;

			const rowBottom = rowTop + ROW_HEIGHT;

			if (rowTop < top) {
				const scrollBy = rowTop - top - (nextRow.index === 0 ? padding.y : 0);

				explorer.scrollRef.current.scrollBy({
					top: scrollBy,
					behavior: 'smooth'
				});
			} else if (rowBottom > scrollRect.bottom) {
				const scrollBy =
					rowBottom -
					scrollRect.height +
					(nextRow.index === rows.length - 1 ? padding.y : 0);

				explorer.scrollRef.current.scrollBy({
					top: scrollBy,
					behavior: 'smooth'
				});
			}
		}
	});

	// Reset resizing cursor
	useWindowEventListener('mouseup', () => {
		setTimeout(() => {
			setResizing(false);
		});
		setDragging(false);
		setIsLeftMouseDown(false);

		if (layout.ref.current) layout.ref.current.style.cursor = '';
		if (tableHeaderRef.current) {
			tableHeaderRef.current.style.overflowX = 'auto';
			tableHeaderRef.current.scrollLeft = scrollLeft.current;
		}
	});

	useWindowEventListener('mousemove', () => {
		if (!isLeftMouseDown) return;

		setDragging(true);
		if (tableHeaderRef.current) tableHeaderRef.current.style.overflowX = 'hidden';
	});

	// Handle table resize
	useResizeObserver({
		ref: tableRef,
		onResize: ({ width }) => {
			if (!width) return;

			const sizing = table
				.getVisibleLeafColumns()
				.reduce(
					(sizing, column) => ({ ...sizing, [column.id]: column.getSize() }),
					{} as ColumnSizingState
				);

			const columnsWidth = Object.values(sizing).reduce((a, b) => a + b, 0) + padding.x * 2;

			if (locked) {
				const newNameSize = (sizing.name ?? 0) + (width - columnsWidth);
				const minNameColSize = table.getColumn('name')?.columnDef.minSize;

				if (minNameColSize !== undefined && newNameSize < minNameColSize) return;

				table.setColumnSizing({
					...columnSizing,
					name: newNameSize
				});
			} else if (Math.abs(width - columnsWidth) < 15) {
				setLocked(true);
			}
		}
	});

	// Set header position and list offset
	useMutationObserver(explorer.scrollRef, () => {
		const view = explorerView.ref.current;
		const scroll = explorer.scrollRef.current;
		if (!view || !scroll) return;
		setTop(explorerView.top ?? parseInt(getComputedStyle(scroll).paddingTop));
		setListOffset(tableRef.current?.offsetTop ?? 0);
	});

	// Set list offset
	useLayoutEffect(() => setListOffset(tableRef.current?.offsetTop ?? 0), []);

	// console.log('is dragging', isRenaming);

	return (
		<div
			ref={tableRef}
			onMouseDown={(e) => {
				// console.log('mousedown');
				e.stopPropagation();
				setIsLeftMouseDown(true);
			}}
		>
			{sized && (
				<>
					<BasicSticky
						scrollElement={explorer.scrollRef.current ?? undefined}
						stickyStyle={{ top, zIndex: 10 }}
						topOffset={-top}
						// Without this the width of the element doesn't get updated
						// when the inspector is toggled
						positionRecheckInterval={100}
					>
						<ContextMenu.Root
							trigger={
								<div
									ref={tableHeaderRef}
									className={clsx(
										'no-scrollbar top-bar-blur overflow-x-auto overscroll-x-none border-y !border-sidebar-divider bg-app/90',
										// Prevent drag scroll when resizing
										dragging && 'overflow-hidden'
									)}
									onScroll={(e) =>
										(scrollLeft.current = e.currentTarget.scrollLeft)
									}
								>
									{table.getHeaderGroups().map((headerGroup) => (
										<div key={headerGroup.id} className="flex w-fit">
											{headerGroup.headers.map((header, i) => {
												const size = header.column.getSize();

												const orderingDirection =
													settings.order &&
													orderingKey(settings.order) === header.id
														? getOrderingDirection(settings.order)
														: null;

												const cellContent = flexRender(
													header.column.columnDef.header,
													header.getContext()
												);

												return (
													<div
														key={header.id}
														className={clsx(
															'relative flex items-center justify-between gap-3 px-4 py-2 text-xs first:pl-[83px]',
															orderingDirection !== null
																? 'text-ink'
																: 'text-ink-dull'
														)}
														style={{
															width:
																i === 0 ||
																i === headerGroup.headers.length - 1
																	? size + padding.x
																	: size
														}}
														onClick={() => {
															if (resizing) return;
															if (header.column.getCanSort()) {
																explorer.settingsStore.order =
																	createOrdering(
																		header.id,
																		orderingDirection === 'Asc'
																			? 'Desc'
																			: 'Asc'
																	);
															}
														}}
													>
														{header.isPlaceholder ? null : (
															<>
																{typeof cellContent === 'string' ? (
																	<HeaderColumnName
																		name={cellContent}
																	/>
																) : (
																	cellContent
																)}

																{orderingDirection === 'Asc' && (
																	<CaretUp className="shrink-0 text-ink-faint" />
																)}

																{orderingDirection === 'Desc' && (
																	<CaretDown className="shrink-0 text-ink-faint" />
																)}

																<div
																	onClick={(e) =>
																		e.stopPropagation()
																	}
																	onMouseDown={(e) => {
																		setResizing(true);
																		setLocked(false);

																		header.getResizeHandler()(
																			e
																		);

																		if (layout.ref.current) {
																			layout.ref.current.style.cursor =
																				'col-resize';
																		}
																	}}
																	onTouchStart={header.getResizeHandler()}
																	className="absolute right-0 h-[70%] w-2 cursor-col-resize border-r border-sidebar-divider"
																/>
															</>
														)}
													</div>
												);
											})}
										</div>
									))}
								</div>
							}
						>
							{table.getAllLeafColumns().map((column) => {
								if (column.id === 'name') return null;
								return (
									<ContextMenu.CheckboxItem
										key={column.id}
										checked={column.getIsVisible()}
										onSelect={column.getToggleVisibilityHandler()}
										label={
											typeof column.columnDef.header === 'string'
												? column.columnDef.header
												: column.id
										}
									/>
								);
							})}
						</ContextMenu.Root>
					</BasicSticky>

					<div
						ref={tableBodyRef}
						className={clsx(
							'no-scrollbar overflow-x-auto overscroll-x-none',
							// Prevent drag scroll
							dragging && 'pointer-events-none'
						)}
						onScroll={(e) => (scrollLeft.current = e.currentTarget.scrollLeft)}
					>
						<div
							className="relative"
							style={{ height: `${rowVirtualizer.getTotalSize()}px` }}
						>
							{virtualRows.map((virtualRow) => {
								const row = rows[virtualRow.index];
								if (!row) return null;

								const selected = explorer.isItemSelected(row.original);
								const cut = isCut(row.original, explorerStore.cutCopyState);

								const previousRow = rows[virtualRow.index - 1];
								const nextRow = rows[virtualRow.index + 1];

								const selectedPrior =
									previousRow && explorer.isItemSelected(previousRow.original);

								const selectedNext =
									nextRow && explorer.isItemSelected(nextRow.original);

								return (
									<div
										key={row.id}
										className="absolute left-0 top-0 min-w-full"
										style={{
											height: virtualRow.size,
											transform: `translateY(${
												virtualRow.start -
												rowVirtualizer.options.scrollMargin
											}px)`
										}}
										onMouseDown={(e) => handleRowClick(e, row)}
										onContextMenu={() => handleRowContextMenu(row)}
									>
										<div
											className={clsx(
												'absolute inset-0 rounded-md border',
												virtualRow.index % 2 === 0 && 'bg-app-darkBox',
												selected
													? 'border-accent !bg-accent/10'
													: 'border-transparent',
												selected &&
													selectedPrior &&
													'rounded-t-none border-t-0 border-t-transparent',
												selected &&
													selectedNext &&
													'rounded-b-none border-b-0 border-b-transparent'
											)}
											style={{ right: padding.x, left: padding.x }}
										>
											{selectedPrior && (
												<div className="absolute inset-x-3 top-0 h-px bg-accent/10" />
											)}
										</div>

										<ListViewItem
											row={row}
											paddingX={padding.x}
											columnSizing={columnSizing}
											columnVisibility={columnVisibility}
											isCut={cut}
											isRenaming={isRenaming}
										/>
									</div>
								);
							})}
						</div>
					</div>
				</>
			)}
		</div>
	);
};
