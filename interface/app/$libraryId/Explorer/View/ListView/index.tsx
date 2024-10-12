import type { ExplorerItem } from '@sd/client';
import type { ColumnSizingState, Row } from '@tanstack/react-table';

import { CaretDown, CaretUp } from '@phosphor-icons/react';
import { flexRender } from '@tanstack/react-table';
import { useVirtualizer } from '@tanstack/react-virtual';
import clsx from 'clsx';
import React, { memo, useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import BasicSticky from 'react-sticky-el';
import { useWindowEventListener } from 'rooks';
import useResizeObserver from 'use-resize-observer';

import { createOrdering, getOrderingDirection, getOrderingKey } from '@sd/client';
import { ContextMenu } from '@sd/ui';
import { TruncatedText } from '~/components';
import { useShortcut } from '~/hooks';
import { isNonEmptyObject } from '~/util';

import { useLayoutContext } from '../../../Layout/Context';
import { useExplorerContext } from '../../Context';
import { getQuickPreviewStore, useQuickPreviewStore } from '../../QuickPreview/store';
import { uniqueId } from '../../util';
import { useExplorerViewContext } from '../Context';
import { useDragScrollable } from '../useDragScrollable';
import { TableContext } from './context';
import { TableRow } from './TableRow';
import { getRangeDirection, Range, useRanges } from './useRanges';
import {
	DEFAULT_LIST_VIEW_ICON_SIZE,
	DEFAULT_LIST_VIEW_TEXT_SIZE,
	LIST_VIEW_ICON_SIZES,
	LIST_VIEW_TEXT_SIZES,
	useTable
} from './useTable';

const ROW_HEIGHT = 37;
const TABLE_HEADER_HEIGHT = 35;
export const TABLE_PADDING_X = 16;
export const TABLE_PADDING_Y = 12;

export const ListView = memo(() => {
	const layout = useLayoutContext();
	const explorer = useExplorerContext();
	const explorerView = useExplorerViewContext();
	const explorerSettings = explorer.useSettingsSnapshot();
	const quickPreview = useQuickPreviewStore();

	const tableRef = useRef<HTMLDivElement>(null);
	const tableHeaderRef = useRef<HTMLDivElement | null>(null);
	const tableBodyRef = useRef<HTMLDivElement>(null);

	const { ref: scrollableRef } = useDragScrollable({ direction: 'up' });

	const [sized, setSized] = useState(false);
	const [initialized, setInitialized] = useState(false);
	const [locked, setLocked] = useState(false);
	const [resizing, setResizing] = useState(false);
	const [top, setTop] = useState(0);
	const [listOffset, setListOffset] = useState(0);
	const [ranges, setRanges] = useState<Range[]>([]);
	const [isLeftMouseDown, setIsLeftMouseDown] = useState(false);

	const { table } = useTable();
	const { columnVisibility, columnSizing } = table.getState();
	const { rows, rowsById } = table.getRowModel();

	const { getRangeByIndex, getRangesByRow, getClosestRange } = useRanges({
		ranges,
		rows: rowsById
	});

	const rowVirtualizer = useVirtualizer({
		count: !explorer.count ? rows.length : Math.max(rows.length, explorer.count),
		getScrollElement: useCallback(() => explorer.scrollRef.current, [explorer.scrollRef]),
		estimateSize: useCallback(() => ROW_HEIGHT, []),
		paddingStart: TABLE_PADDING_Y,
		paddingEnd: TABLE_PADDING_Y + (explorerView.scrollPadding?.bottom ?? 0),
		scrollMargin: listOffset,
		overscan: explorer.overscan ?? 10,
		scrollPaddingStart: explorerView.scrollPadding?.top,
		scrollPaddingEnd: TABLE_HEADER_HEIGHT + (explorerView.scrollPadding?.bottom ?? 0)
	});

	const virtualRows = rowVirtualizer.getVirtualItems();

	const handleRowClick = (
		e: React.MouseEvent<HTMLDivElement, MouseEvent>,
		row: Row<ExplorerItem>
	) => {
		// Ensure mouse click is with left button
		if (e.button !== 0) return;

		const rowIndex = row.index;
		const item = row.original;

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
				for (let i = backRange.sorted.start.index; i <= backRange.sorted.end.index; i++) {
					const index = backRange.sorted.start.index + i;

					if (index === range.start.index) continue;

					const row = rows[index];

					if (row) explorer.removeSelectedItem(row.original);
				}

				_ranges = _ranges.filter((_, i) => i !== backRange.index);
			}

			for (
				let i = 0;
				i < Math.abs(range.end.index - rowIndex) + (changeDirection ? 1 : 0);
				i++
			) {
				if (!range.direction || direction === range.direction) i += 1;

				const index = range.end.index + (direction === 'down' ? i : -i);

				const row = rows[index];

				if (!row) continue;

				const item = row.original;

				if (uniqueId(item) === uniqueId(range.start.original)) continue;

				if (
					!range.direction ||
					direction === range.direction ||
					(changeDirection &&
						(range.direction === 'down'
							? index < range.start.index
							: index > range.start.index))
				) {
					explorer.addSelectedItem(item);
				} else {
					explorer.removeSelectedItem(item);
				}
			}

			let newRangeEnd = item;
			let removeRangeIndex: number | null = null;

			for (let i = 0; i < _ranges.length - 1; i++) {
				const range = getRangeByIndex(i);

				if (!range) continue;

				if (rowIndex >= range.sorted.start.index && rowIndex <= range.sorted.end.index) {
					const removableRowsCount = Math.abs(
						(direction === 'down' ? range.sorted.end.index : range.sorted.start.index) -
							rowIndex
					);

					for (let i = 1; i <= removableRowsCount; i++) {
						const index = rowIndex + (direction === 'down' ? i : -i);

						const row = rows[index];

						if (row) explorer.removeSelectedItem(row.original);
					}

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
	};

	const handleRowContextMenu = (row: Row<ExplorerItem>) => {
		if (explorerView.contextMenu === undefined) return;

		const item = row.original;

		if (!explorer.isItemSelected(item)) {
			explorer.resetSelectedItems([item]);
			const hash = uniqueId(item);
			setRanges([[hash, hash]]);
		}
	};

	const scrollToRow = useCallback(
		(row: Row<ExplorerItem>) => {
			rowVirtualizer.scrollToIndex(row.index, {
				align: row.index === 0 ? 'end' : row.index === rows.length - 1 ? 'start' : 'auto'
			});
		},
		[rowVirtualizer, rows.length]
	);

	const keyboardHandler = (e: KeyboardEvent, direction: 'ArrowDown' | 'ArrowUp') => {
		if (!explorerView.selectable) return;

		e.preventDefault();

		const range = getRangeByIndex(ranges.length - 1);

		if (explorer.selectedItems.size === 0) {
			const item = rows[0]?.original;
			if (item) {
				explorer.addSelectedItem(item);
				setRanges([[uniqueId(item), uniqueId(item)]]);
			}
			return;
		}

		if (!range) return;

		const keyDirection = direction === 'ArrowDown' ? 'down' : 'up';

		const nextRow = rows[range.end.index + (keyDirection === 'up' ? -1 : 1)];

		if (!nextRow) return;

		const item = nextRow.original;

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
								: [uniqueId(frontRange.start.original), uniqueId(nextRow.original)];
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
						direction === 'down' ? closestRange.sorted.end : closestRange.sorted.start;
				}

				if (backRange && frontRange) {
					let _ranges = [...ranges];

					const backRangeStart = backRange.start.original;

					const backRangeEnd =
						rangeEndRow.index < backRange.sorted.start.index ||
						rangeEndRow.index > backRange.sorted.end.index
							? rangeEndRow.original
							: backRange.end.original;

					_ranges[backRange.index] = [uniqueId(backRangeStart), uniqueId(backRangeEnd)];

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

		scrollToRow(nextRow);
	};

	useEffect(() => setRanges([]), [explorerSettings.order]);

	useEffect(() => {
		if (explorer.selectedItems.size === 0) setRanges([]);
	}, [explorer.selectedItems]);

	useEffect(() => {
		// Reset icon size if it's not a valid size
		if (!LIST_VIEW_ICON_SIZES[explorerSettings.listViewIconSize]) {
			explorer.settingsStore.listViewIconSize = DEFAULT_LIST_VIEW_ICON_SIZE;
		}

		// Reset text size if it's not a valid size
		if (!LIST_VIEW_TEXT_SIZES[explorerSettings.listViewTextSize]) {
			explorer.settingsStore.listViewTextSize = DEFAULT_LIST_VIEW_TEXT_SIZE;
		}
	}, [
		explorer.settingsStore,
		explorerSettings.listViewIconSize,
		explorerSettings.listViewTextSize
	]);

	useEffect(() => {
		if (!getQuickPreviewStore().open || explorer.selectedItems.size !== 1) return;

		const [item] = [...explorer.selectedItems];
		if (!item) return;

		const itemId = uniqueId(item);
		setRanges([[itemId, itemId]]);
	}, [explorer.selectedItems]);

	useEffect(() => {
		if (initialized || !sized || !explorer.count || explorer.selectedItems.size === 0) {
			if (explorer.selectedItems.size === 0 && !initialized) setInitialized(true);
			return;
		}

		const rows = [...explorer.selectedItems]
			.reduce((rows, item) => {
				const row = rowsById[uniqueId(item)];
				if (row) rows.push(row);
				return rows;
			}, [] as Row<ExplorerItem>[])
			.sort((a, b) => a.index - b.index);

		const lastRow = rows[rows.length - 1];
		if (!lastRow) return;

		scrollToRow(lastRow);
		setRanges(rows.map(row => [uniqueId(row.original), uniqueId(row.original)] as Range));
		setInitialized(true);
	}, [explorer.count, explorer.selectedItems, initialized, rowsById, scrollToRow, sized]);

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
		const columnsWidth = Object.values(sizing).reduce((a, b) => a + b, 0) + TABLE_PADDING_X * 2;

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
	}, [columnSizing, columnVisibility, sized, table]);

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

		if (!table || !header || !body || quickPreview.open) return;

		const handleWheel = (event: WheelEvent) => {
			if (Math.abs(event.deltaX) < Math.abs(event.deltaY)) return;
			event.preventDefault();
			header.scrollLeft += event.deltaX;
			body.scrollLeft += event.deltaX;
		};

		const handleScroll = (element: HTMLDivElement) => {
			if (isLeftMouseDown) return;
			// Sorting sometimes resets scrollLeft
			// so we reset it here in case it does
			// to keep the scroll in sync
			// TODO: Find a better solution
			header.scrollLeft = element.scrollLeft;
			body.scrollLeft = element.scrollLeft;
		};

		table.addEventListener('wheel', handleWheel);
		header.addEventListener('scroll', () => handleScroll(header));
		body.addEventListener('scroll', () => handleScroll(body));

		return () => {
			table.removeEventListener('wheel', handleWheel);
			header.addEventListener('scroll', () => handleScroll(header));
			body.addEventListener('scroll', () => handleScroll(body));
		};
	}, [sized, isLeftMouseDown, quickPreview.open]);

	useShortcut('explorerUp', e => {
		keyboardHandler(e, 'ArrowUp');
	});

	useShortcut('explorerDown', e => {
		keyboardHandler(e, 'ArrowDown');
	});

	// Reset resizing cursor
	useWindowEventListener('mouseup', () => {
		// We timeout the reset so the col sorting
		// doesn't get triggered on mouse up
		setTimeout(() => {
			setResizing(false);
			setIsLeftMouseDown(false);
			if (layout.ref.current) layout.ref.current.style.cursor = '';
		});
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

			const columnsWidth =
				Object.values(sizing).reduce((a, b) => a + b, 0) + TABLE_PADDING_X * 2;

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
	useEffect(() => {
		const element = explorer.scrollRef.current;
		if (!element) return;

		const observer = new MutationObserver(() => {
			setTop(
				explorerView.scrollPadding?.top ??
					parseInt(getComputedStyle(element).paddingTop) +
						element.getBoundingClientRect().top
			);
			setListOffset(tableRef.current?.offsetTop ?? 0);
		});

		observer.observe(element, {
			attributes: true,
			subtree: true
		});

		return () => observer.disconnect();
	}, [explorer.scrollRef, explorerView.scrollPadding?.top]);

	// Set list offset
	useLayoutEffect(() => setListOffset(tableRef.current?.offsetTop ?? 0), []);

	// Handle active item selection
	// TODO: This is a temporary solution
	useEffect(() => {
		return () => {
			const firstRange = getRangeByIndex(0);
			if (!firstRange) return;

			const lastRange = getRangeByIndex(ranges.length - 1);
			if (!lastRange) return;

			const firstItem = firstRange.start.original;
			const lastItem = lastRange.end.original;

			explorerView.updateFirstActiveItem(explorer.getItemUniqueId(firstItem));
			explorerView.updateActiveItem(explorer.getItemUniqueId(lastItem));
		};
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [
		ranges,
		getRangeByIndex,
		explorerView.updateFirstActiveItem,
		explorerView.updateActiveItem,
		explorer.getItemUniqueId
	]);

	return (
		<TableContext.Provider value={{ columnSizing }}>
			<div
				ref={tableRef}
				onMouseDown={e => {
					if (e.button !== 0) return;
					e.stopPropagation();
					setIsLeftMouseDown(true);
				}}
				className={clsx(!initialized && 'invisible')}
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
										ref={element => {
											tableHeaderRef.current = element;
											scrollableRef(element);
										}}
										className={clsx(
											'top-bar-blur !border-sidebar-divider bg-app/90',
											explorerView.listViewOptions?.hideHeaderBorder
												? 'border-b'
												: 'border-y',
											// Prevent drag scroll
											isLeftMouseDown
												? 'overflow-hidden'
												: 'no-scrollbar overflow-x-auto overscroll-x-none'
										)}
										style={{ height: TABLE_HEADER_HEIGHT }}
									>
										{table.getHeaderGroups().map(headerGroup => (
											<div key={headerGroup.id} className="flex w-fit">
												{headerGroup.headers.map((header, i) => {
													const size = header.column.getSize();

													const orderKey =
														explorerSettings.order &&
														getOrderingKey(explorerSettings.order);

													const orderingDirection =
														orderKey &&
														explorerSettings.order &&
														(orderKey.startsWith('object.')
															? orderKey.split('object.')[1] ===
																header.id
															: orderKey === header.id)
															? getOrderingDirection(
																	explorerSettings.order
																)
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
																	i ===
																		headerGroup.headers.length -
																			1
																		? size + TABLE_PADDING_X
																		: size
															}}
															onClick={() => {
																if (resizing) return;

																// Split table into smaller parts
																// cause this looks hideous
																const orderKey =
																	explorer.orderingKeys?.options.find(
																		o => {
																			if (
																				typeof o.value !==
																				'string'
																			)
																				return;

																			const value =
																				o.value as string;

																			return value.startsWith(
																				'object.'
																			)
																				? value.split(
																						'object.'
																					)[1] ===
																						header.id
																				: value ===
																						header.id;
																		}
																	);

																if (!orderKey) return;

																explorer.settingsStore.order =
																	createOrdering(
																		orderKey.value,
																		orderingDirection === 'Asc'
																			? 'Desc'
																			: 'Asc'
																	);
															}}
														>
															{header.isPlaceholder ? null : (
																<>
																	<TruncatedText>
																		{cellContent}
																	</TruncatedText>

																	{orderingDirection ===
																		'Asc' && (
																		<CaretUp className="shrink-0 text-ink-faint" />
																	)}

																	{orderingDirection ===
																		'Desc' && (
																		<CaretDown className="shrink-0 text-ink-faint" />
																	)}

																	<div
																		onMouseDown={e => {
																			setResizing(true);
																			setLocked(false);

																			header.getResizeHandler()(
																				e
																			);

																			if (
																				layout.ref.current
																			) {
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
								{table.getAllLeafColumns().map(column => {
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
								// Prevent drag scroll
								isLeftMouseDown
									? 'overflow-hidden'
									: 'no-scrollbar overflow-x-auto overscroll-x-none'
							)}
						>
							<div
								className="relative"
								style={{ height: `${rowVirtualizer.getTotalSize()}px` }}
							>
								<div
									className="absolute left-0 top-0 min-w-full"
									style={{
										transform: `translateY(${
											(virtualRows[0]?.start ?? 0) -
											rowVirtualizer.options.scrollMargin
										}px)`
									}}
								>
									{virtualRows.map(virtualRow => {
										const row = rows[virtualRow.index];
										if (!row) return null;

										const previousRow = rows[virtualRow.index - 1];
										const nextRow = rows[virtualRow.index + 1];

										return (
											<div
												key={virtualRow.key}
												data-index={virtualRow.index}
												ref={rowVirtualizer.measureElement}
												className="relative"
												onMouseDown={e => handleRowClick(e, row)}
												onContextMenu={() => handleRowContextMenu(row)}
											>
												<TableRow
													row={row}
													previousRow={previousRow}
													nextRow={nextRow}
												/>
											</div>
										);
									})}
								</div>
							</div>
						</div>
					</>
				)}
			</div>
		</TableContext.Provider>
	);
});
