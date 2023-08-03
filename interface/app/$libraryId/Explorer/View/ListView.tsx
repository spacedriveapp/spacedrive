import {
	ColumnDef,
	ColumnSizingState,
	Row,
	flexRender,
	getCoreRowModel,
	useReactTable
} from '@tanstack/react-table';
import { useVirtualizer } from '@tanstack/react-virtual';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { CaretDown, CaretUp } from 'phosphor-react';
import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { ScrollSync, ScrollSyncPane } from 'react-scroll-sync';
import { useBoundingclientrect, useKey, useMutationObserver, useWindowEventListener } from 'rooks';
import useResizeObserver from 'use-resize-observer';
import {
	ExplorerItem,
	FilePath,
	ObjectKind,
	byteSize,
	getExplorerItemData,
	getItemFilePath,
	getItemLocation,
	getItemObject,
	isPath
} from '@sd/client';
import { Tooltip } from '@sd/ui';
import { useIsTextTruncated, useScrolled } from '~/hooks';
import { ViewItem } from '.';
import { useLayoutContext } from '../../Layout/Context';
import FileThumb from '../FilePath/Thumb';
import { InfoPill } from '../Inspector';
import { useExplorerViewContext } from '../ViewContext';
import { FilePathSearchOrderingKeys, getExplorerStore, isCut, useExplorerStore } from '../store';
import RenamableItemText from './RenamableItemText';

interface ListViewItemProps {
	row: Row<ExplorerItem>;
	columnSizing: ColumnSizingState;
	paddingX: number;
	selected: boolean;
	cut: boolean;
}

const ListViewItem = memo((props: ListViewItemProps) => {
	return (
		<ViewItem data={props.row.original} className="w-full">
			<div role="row" className="flex h-full items-center">
				{props.row.getVisibleCells().map((cell, i, cells) => {
					return (
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
					);
				})}
			</div>
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

export default () => {
	const explorerStore = useExplorerStore();
	const explorerView = useExplorerViewContext();
	const layout = useLayoutContext();

	const tableRef = useRef<HTMLDivElement>(null);
	const tableHeaderRef = useRef<HTMLDivElement>(null);
	const tableBodyRef = useRef<HTMLDivElement>(null);

	const [sized, setSized] = useState(false);
	const [locked, setLocked] = useState(false);
	const [resizing, setResizing] = useState(false);
	const [columnSizing, setColumnSizing] = useState<ColumnSizingState>({});
	const [listOffset, setListOffset] = useState(0);
	const [ranges, setRanges] = useState<[number, number][]>([]);

	const top =
		(explorerView.top || 0) +
		(explorerView.scrollRef.current
			? parseInt(getComputedStyle(explorerView.scrollRef.current).paddingTop)
			: 0);

	const { isScrolled } = useScrolled(
		explorerView.scrollRef,
		sized ? listOffset - top : undefined
	);

	const paddingX =
		(typeof explorerView.padding === 'object'
			? explorerView.padding.x
			: explorerView.padding) || 16;

	const paddingY =
		(typeof explorerView.padding === 'object'
			? explorerView.padding.y
			: explorerView.padding) || 12;

	const scrollBarWidth = 8;
	const rowHeight = 45;

	const { width: tableWidth = 0 } = useResizeObserver({ ref: tableRef });
	const { width: headerWidth = 0 } = useResizeObserver({ ref: tableHeaderRef });

	const getFileName = (path: FilePath) => `${path.name}${path.extension && `.${path.extension}`}`;

	const columns = useMemo<ColumnDef<ExplorerItem>[]>(
		() => [
			{
				id: 'name',
				header: 'Name',
				minSize: 200,
				size: 350,
				maxSize: undefined,
				meta: { className: '!overflow-visible !text-ink' },
				accessorFn: (file) => {
					const locationData = getItemLocation(file);
					const filePathData = getItemFilePath(file);
					return locationData
						? locationData.name
						: filePathData && getFileName(filePathData);
				},
				cell: (cell) => {
					const file = cell.row.original;

					const selectedId = Array.isArray(explorerView.selected)
						? explorerView.selected[0]
						: explorerView.selected;

					const selected = selectedId === cell.row.original.item.id;

					const cut = isCut(file.item.id);

					return (
						<div className="relative flex items-center">
							<div className="mr-[10px] flex h-6 w-12 shrink-0 items-center justify-center">
								<FileThumb
									data={file}
									size={35}
									className={clsx(cut && 'opacity-60')}
								/>
							</div>
							<RenamableItemText
								allowHighlight={false}
								item={file}
								selected={selected}
								disabled={
									!selected ||
									(Array.isArray(explorerView.selected) &&
										explorerView.selected.length > 1)
								}
								style={{ maxHeight: 36 }}
							/>
						</div>
					);
				}
			},
			{
				id: 'kind',
				header: 'Type',
				enableSorting: false,
				accessorFn: (file) => {
					return isPath(file) && file.item.is_dir
						? 'Folder'
						: ObjectKind[getItemObject(file)?.kind || 0];
				},
				cell: (cell) => {
					const file = cell.row.original;
					return (
						<InfoPill className="bg-app-button/50">
							{isPath(file) && file.item.is_dir
								? 'Folder'
								: ObjectKind[getItemObject(file)?.kind || 0]}
						</InfoPill>
					);
				}
			},
			{
				id: 'sizeInBytes',
				header: 'Size',
				size: 100,
				accessorFn: (file) => {
					const file_path = getItemFilePath(file);
					if (!file_path || !file_path.size_in_bytes_bytes) return;

					return byteSize(file_path.size_in_bytes_bytes);
				}
			},
			{
				id: 'dateCreated',
				header: 'Date Created',
				accessorFn: (file) => dayjs(file.item.date_created).format('MMM Do YYYY')
			},
			{
				id: 'dateModified',
				header: 'Date Modified',
				accessorFn: (file) =>
					dayjs(getItemFilePath(file)?.date_modified).format('MMM Do YYYY')
			},
			{
				id: 'dateIndexed',
				header: 'Date Indexed',
				accessorFn: (file) =>
					dayjs(getItemFilePath(file)?.date_indexed).format('MMM Do YYYY')
			},
			{
				id: 'dateAccessed',
				header: 'Date Accessed',
				accessorFn: (file) =>
					getItemObject(file)?.date_accessed &&
					dayjs(getItemObject(file)?.date_accessed).format('MMM Do YYYY')
			},
			{
				header: 'Content ID',
				enableSorting: false,
				size: 180,
				accessorFn: (file) => getExplorerItemData(file).casId
			},
			{
				header: 'Object ID',
				enableSorting: false,
				size: 180,
				accessorFn: (file) => getItemObject(file)?.pub_id
			}
		],
		[explorerView.selected, explorerStore.cutCopyState.sourcePathId]
	);

	const table = useReactTable({
		data: explorerView.items || [],
		columns,
		defaultColumn: { minSize: 100, maxSize: 250 },
		state: { columnSizing },
		onColumnSizingChange: setColumnSizing,
		columnResizeMode: 'onChange',
		getCoreRowModel: getCoreRowModel(),
		getRowId: (row) => String(row.item.id)
	});

	const tableLength = table.getTotalSize();
	const rows = useMemo(() => table.getRowModel().rows, [explorerView.items]);

	const rowVirtualizer = useVirtualizer({
		count: explorerView.items ? rows.length : 100,
		getScrollElement: () => explorerView.scrollRef.current,
		estimateSize: () => rowHeight,
		paddingStart: paddingY + (isScrolled ? 35 : 0),
		paddingEnd: paddingY,
		scrollMargin: listOffset
	});

	const virtualRows = rowVirtualizer.getVirtualItems();

	function isSelected(id: number) {
		return typeof explorerView.selected === 'object'
			? explorerView.selected.has(id)
			: explorerView.selected === id;
	}

	function getRangeDirection(start: number, end: number) {
		return start < end ? ('down' as const) : start > end ? ('up' as const) : null;
	}

	function getRangeByIndex(index: number) {
		const range = ranges[index];

		if (!range) return;

		const rangeRows = getRangeRows(range);

		if (!rangeRows) return;

		const direction = getRangeDirection(rangeRows.start.index, rangeRows.end.index);

		return { ...rangeRows, direction, index };
	}

	function getRangesByRow({ index }: Row<ExplorerItem>) {
		const _ranges = ranges.reduce<NonNullable<ReturnType<typeof getRangeByIndex>>[]>(
			(ranges, range, i) => {
				const rangeRows = getRangeRows(range);

				if (!rangeRows) return ranges;

				if (index >= rangeRows.sorted.start.index && index <= rangeRows.sorted.end.index) {
					const range = getRangeByIndex(i);
					return range ? [...ranges, range] : ranges;
				}

				return ranges;
			},
			[]
		);

		return _ranges;
	}

	function getRangeRows(range: [number, number]) {
		const { rowsById } = table.getCoreRowModel();

		const rangeRows = range
			.map((id) => rowsById[id])
			.filter((row): row is Row<ExplorerItem> => Boolean(row));

		const [start, end] = rangeRows;

		const [sortedStart, sortedEnd] = [...rangeRows].sort((a, b) => a.index - b.index);

		if (!start || !end || !sortedStart || !sortedEnd) return;

		return { start, end, sorted: { start: sortedStart, end: sortedEnd } };
	}

	function sortRanges(ranges: [number, number][]) {
		return ranges
			.map((range, i) => {
				const rows = getRangeRows(range);

				if (!rows) return;

				return {
					index: i,
					...rows
				};
			})
			.filter(
				(
					range
				): range is NonNullable<ReturnType<typeof getRangeRows>> & { index: number } =>
					Boolean(range)
			)
			.sort((a, b) => a.sorted.start.index - b.sorted.start.index);
	}

	function getClosestRange(
		rangeIndex: number,
		options: {
			direction?: 'up' | 'down';
			maxRowDifference?: number;
			ranges?: [number, number][];
		} = {}
	) {
		const range = getRangeByIndex(rangeIndex);

		let _ranges = sortRanges(options.ranges || ranges);

		if (range) {
			_ranges = _ranges.filter(
				(_range) =>
					range.index === _range.index ||
					range.sorted.start.index < _range.sorted.start.index ||
					range.sorted.end.index > _range.sorted.end.index
			);
		}

		const targetRangeIndex = _ranges.findIndex(({ index }) => rangeIndex === index);

		const targetRange = _ranges[targetRangeIndex];

		if (!targetRange) return;

		const closestRange =
			options.direction === 'down'
				? _ranges[targetRangeIndex + 1]
				: options.direction === 'up'
				? _ranges[targetRangeIndex - 1]
				: _ranges[targetRangeIndex + 1] || _ranges[targetRangeIndex - 1];

		if (!closestRange) return;

		const direction = options.direction || (_ranges[targetRangeIndex + 1] ? 'down' : 'up');

		const rowDifference =
			direction === 'down'
				? closestRange.sorted.start.index - 1 - targetRange.sorted.end.index
				: targetRange.sorted.start.index - (closestRange.sorted.end.index + 1);

		if (options.maxRowDifference !== undefined && rowDifference > options.maxRowDifference)
			return;

		return {
			...closestRange,
			direction,
			rowDifference
		};
	}

	function handleRowClick(
		e: React.MouseEvent<HTMLDivElement, MouseEvent>,
		row: Row<ExplorerItem>
	) {
		if (!explorerView.onSelectedChange || e.button !== 0) return;

		const rowIndex = row.index;
		const itemId = row.original.item.id;

		if (typeof explorerView.selected === 'object') {
			if (e.shiftKey) {
				const { rows } = table.getCoreRowModel();

				const range = getRangeByIndex(ranges.length - 1);

				if (!range) {
					const ids = [...Array(rowIndex + 1)].reduce<number[]>((ids, _, i) => {
						const id = rows[i]?.original.item.id;
						if (id !== null && id !== undefined) return [...ids, id];
						return ids;
					}, []);

					const [rangeStartId] = ids;

					if (rangeStartId !== null && rangeStartId !== undefined) {
						setRanges([[rangeStartId, itemId]]);
					}

					return explorerView.onSelectedChange(new Set(ids));
				}

				const direction = getRangeDirection(range.end.index, rowIndex);

				if (!direction) return;

				const changeDirection =
					!!range.direction &&
					range.direction !== direction &&
					(direction === 'down'
						? rowIndex > range.start.index
						: rowIndex < range.start.index);

				const selected = new Set(explorerView.selected);

				let _ranges = ranges;

				const [backRange, frontRange] = getRangesByRow(range.start);

				if (backRange && frontRange) {
					[
						...Array(backRange.sorted.end.index - backRange.sorted.start.index + 1)
					].forEach((_, i) => {
						const index = backRange.sorted.start.index + i;

						if (index === range.start.index) return;

						const row = rows[index];

						if (row) selected.delete(row.original.item.id);
					});

					_ranges = _ranges.filter((_, i) => i !== backRange.index);
				}

				[
					...Array(Math.abs(range.end.index - rowIndex) + (changeDirection ? 1 : 0))
				].forEach((_, i) => {
					if (!range.direction || direction === range.direction) i += 1;

					const index = range.end.index + (direction === 'down' ? i : -i);

					const row = rows[index];

					if (row) {
						const id = row.original.item.id;

						if (id === range.start.original.item.id) return;

						if (
							!range.direction ||
							direction === range.direction ||
							(changeDirection &&
								(range.direction === 'down'
									? index < range.start.index
									: index > range.start.index))
						) {
							selected.add(id);
						} else selected.delete(id);
					}
				});

				let newRangeEndId = itemId;
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

							if (row) selected.delete(row.original.item.id);
						});

						removeRangeIndex = i;
						break;
					} else if (direction === 'down' && rowIndex + 1 === range.sorted.start.index) {
						newRangeEndId = range.sorted.end.original.item.id;
						removeRangeIndex = i;
						break;
					} else if (direction === 'up' && rowIndex - 1 === range.sorted.end.index) {
						newRangeEndId = range.sorted.start.original.item.id;
						removeRangeIndex = i;
						break;
					}
				}

				if (removeRangeIndex !== null) {
					_ranges = _ranges.filter((_, i) => i !== removeRangeIndex);
				}

				setRanges([
					..._ranges.slice(0, _ranges.length - 1),
					[range.start.original.item.id, newRangeEndId]
				]);

				explorerView.onSelectedChange(selected);
			} else if (e.metaKey) {
				const { rows } = table.getCoreRowModel();

				const selected = new Set(explorerView.selected);

				if (selected.has(itemId)) {
					selected.delete(itemId);

					const rowRanges = getRangesByRow(row);

					const range = rowRanges[0] || rowRanges[1];

					if (range) {
						const rangeStartId = range.sorted.start.original.item.id;
						const rangeEndId = range.sorted.end.original.item.id;

						if (rangeStartId === rangeEndId) {
							const closestRange = getClosestRange(range.index);
							if (closestRange) {
								const _ranges = ranges.filter(
									(_, i) => i !== closestRange.index && i !== range.index
								);

								const startId = closestRange.sorted.start.original.item.id;
								const endId = closestRange.sorted.end.original.item.id;

								setRanges([
									..._ranges,
									[
										closestRange.direction === 'down' ? startId : endId,
										closestRange.direction === 'down' ? endId : startId
									]
								]);
							} else {
								setRanges([]);
							}
						} else if (rangeStartId === itemId || rangeEndId === itemId) {
							const _ranges = ranges.filter(
								(_, i) => i !== range.index && i !== rowRanges[1]?.index
							);

							const startId =
								rows[
									rangeStartId === itemId
										? range.sorted.start.index + 1
										: range.sorted.end.index - 1
								]?.original.item.id;

							if (startId !== undefined) {
								const endId = rangeStartId === itemId ? rangeEndId : rangeStartId;

								setRanges([..._ranges, [startId, endId] as [number, number]]);
							}
						} else {
							const rowBefore = rows[row.index - 1];
							const rowAfter = rows[row.index + 1];

							if (rowBefore && rowAfter) {
								const firstRange = [rangeStartId, rowBefore.original.item.id];

								const secondRange = [rowAfter.original.item.id, rangeEndId];

								const _ranges = ranges.filter(
									(_, i) => i !== range.index && i !== rowRanges[1]?.index
								);

								setRanges([
									..._ranges,
									firstRange as [number, number],
									secondRange as [number, number]
								]);
							}
						}
					}
				} else {
					selected.add(itemId);

					const _ranges = [...ranges, [itemId, itemId]] as [number, number][];

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
								rangeUp.sorted.start.original.item.id,
								rangeDown.sorted.end.original.item.id
							],
							[itemId, itemId]
						]);
					} else if (rangeUp || rangeDown) {
						const closestRange = rangeDown || rangeUp;

						if (closestRange) {
							const _ranges = ranges.filter((_, i) => i !== closestRange.index);

							setRanges([
								..._ranges,
								[
									itemId,
									closestRange.direction === 'down'
										? closestRange.sorted.end.original.item.id
										: closestRange.sorted.start.original.item.id
								]
							]);
						}
					} else {
						setRanges([...ranges, [itemId, itemId]]);
					}
				}

				explorerView.onSelectedChange(selected);
			} else {
				explorerView.onSelectedChange(new Set([itemId]));
				setRanges([[itemId, itemId]]);
			}
		} else explorerView.onSelectedChange(itemId);
	}

	function handleRowContextMenu(row: Row<ExplorerItem>) {
		if (!explorerView.onSelectedChange || explorerView.contextMenu === undefined) return;

		const itemId = row.original.item.id;

		if (!isSelected(itemId)) {
			explorerView.onSelectedChange(
				typeof explorerView.selected === 'object' ? new Set([itemId]) : itemId
			);
			setRanges([[itemId, itemId]]);
		}
	}

	function handleResize() {
		if (locked && Object.keys(columnSizing).length > 0) {
			table.setColumnSizing((sizing) => {
				const nameSize = sizing.name;
				const nameColumnMinSize = table.getColumn('name')?.columnDef.minSize;
				const newNameSize =
					(nameSize || 0) + tableWidth - paddingX * 2 - scrollBarWidth - tableLength;

				return {
					...sizing,
					...(nameSize !== undefined && nameColumnMinSize !== undefined
						? {
								name:
									newNameSize >= nameColumnMinSize
										? newNameSize
										: nameColumnMinSize
						  }
						: {})
				};
			});
		} else if (Math.abs(tableWidth - (tableLength + paddingX * 2 + scrollBarWidth)) < 15) {
			setLocked(true);
		}
	}

	useEffect(() => handleResize(), [tableWidth]);

	useEffect(() => setRanges([]), [explorerView.items]);

	// Measure initial column widths
	useEffect(() => {
		if (tableRef.current) {
			const columns = table.getAllColumns();
			const sizings = columns.reduce(
				(sizings, column) => ({ ...sizings, [column.id]: column.getSize() }),
				{} as ColumnSizingState
			);
			const scrollWidth = tableRef.current.offsetWidth;
			const sizingsSum = Object.values(sizings).reduce((a, b) => a + b, 0);

			if (sizingsSum < scrollWidth) {
				const nameColSize = sizings.name;
				const nameWidth =
					scrollWidth - paddingX * 2 - scrollBarWidth - (sizingsSum - (nameColSize || 0));

				table.setColumnSizing({ ...sizings, name: nameWidth });
				setLocked(true);
			} else table.setColumnSizing(sizings);

			setSized(true);
		}
	}, []);

	// Load more items
	useEffect(() => {
		if (explorerView.onLoadMore) {
			const lastRow = virtualRows[virtualRows.length - 1];
			if (lastRow) {
				const rowsBeforeLoadMore = explorerView.rowsBeforeLoadMore || 1;

				const loadMoreOnIndex =
					rowsBeforeLoadMore > rows.length ||
					lastRow.index > rows.length - rowsBeforeLoadMore
						? rows.length - 1
						: rows.length - rowsBeforeLoadMore;

				if (lastRow.index === loadMoreOnIndex) explorerView.onLoadMore();
			}
		}
	}, [virtualRows, rows.length, explorerView.rowsBeforeLoadMore, explorerView.onLoadMore]);

	useKey(['ArrowUp', 'ArrowDown'], (e) => {
		if (!explorerView.selectable || !explorerView.onSelectedChange) return;

		e.preventDefault();

		const range = getRangeByIndex(ranges.length - 1);

		if (!range) return;

		const keyDirection = e.key === 'ArrowDown' ? 'down' : 'up';

		const nextRow = rows[range.end.index + (keyDirection === 'up' ? -1 : 1)];

		if (!nextRow) return;

		const itemId = nextRow.original.item.id;

		if (typeof explorerView.selected === 'object') {
			if (e.shiftKey) {
				const selected = new Set(explorerView.selected);

				const direction = range.direction || keyDirection;

				const [backRange, frontRange] = getRangesByRow(range.start);

				if (
					range.direction
						? keyDirection !== range.direction
						: backRange?.direction &&
						  (backRange?.sorted.start.index === frontRange?.sorted.start.index ||
								backRange?.sorted.end.index === frontRange?.sorted.end.index)
				) {
					selected.delete(range.end.original.item.id);

					if (backRange && frontRange) {
						let _ranges = [...ranges];

						_ranges[backRange.index] = [
							backRange.direction !== keyDirection
								? backRange.start.original.item.id
								: nextRow.original.item.id,

							backRange.direction !== keyDirection
								? nextRow.original.item.id
								: backRange.end.original.item.id
						];

						if (
							(!range.direction && keyDirection === 'down') ||
							nextRow.index === backRange.start.index
						) {
							_ranges = _ranges.filter((_, i) => i !== frontRange.index);
						} else {
							_ranges[frontRange.index] =
								frontRange.start.index === frontRange.end.index
									? [nextRow.original.item.id, nextRow.original.item.id]
									: [frontRange.start.original.item.id, nextRow.original.item.id];
						}

						setRanges(_ranges);
					} else {
						setRanges([
							...ranges.slice(0, ranges.length - 1),
							[range.start.original.item.id, nextRow.original.item.id]
						]);
					}
				} else {
					selected.add(itemId);

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

						const backRangeStartId =
							direction === 'down' || rangeEndRow.index > backRange.sorted.start.index
								? backRange.start.original.item.id
								: rangeEndRow.original.item.id;

						const backRangeEndId =
							direction === 'up' || rangeEndRow.index < backRange.sorted.end.index
								? backRange.end.original.item.id
								: rangeEndRow.original.item.id;

						_ranges[backRange.index] = [backRangeStartId, backRangeEndId];

						if (
							rangeEndRow.original.item.id === backRangeStartId ||
							rangeEndRow.original.item.id === backRangeEndId
						) {
							_ranges[backRange.index] =
								rangeEndRow.original.item.id === backRangeStartId
									? [backRangeEndId, backRangeStartId]
									: [backRangeStartId, backRangeEndId];
						}

						_ranges[frontRange.index] = [
							frontRange.start.original.item.id,
							rangeEndRow.original.item.id
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
							[range.start.original.item.id, rangeEndRow.original.item.id]
						]);
					}
				}

				explorerView.onSelectedChange(selected);
			} else {
				explorerView.onSelectedChange(new Set([itemId]));
				setRanges([[itemId, itemId]]);
			}
		} else explorerView.onSelectedChange(itemId);

		if (explorerView.scrollRef.current) {
			const tableBodyRect = tableBodyRef.current?.getBoundingClientRect();
			const scrollRect = explorerView.scrollRef.current.getBoundingClientRect();

			const paddingTop = parseInt(
				getComputedStyle(explorerView.scrollRef.current).paddingTop
			);

			const top =
				(explorerView.top ? paddingTop + explorerView.top : paddingTop) +
				scrollRect.top +
				(isScrolled ? 35 : 0);

			const rowTop =
				nextRow.index * rowHeight +
				rowVirtualizer.options.paddingStart +
				(tableBodyRect?.top || 0) +
				scrollRect.top;

			const rowBottom = rowTop + rowHeight;

			if (rowTop < top) {
				const scrollBy = rowTop - top - (nextRow.index === 0 ? paddingY : 0);

				explorerView.scrollRef.current.scrollBy({
					top: scrollBy,
					behavior: 'smooth'
				});
			} else if (rowBottom > scrollRect.bottom) {
				const scrollBy =
					rowBottom -
					scrollRect.height +
					(nextRow.index === rows.length - 1 ? paddingY : 0);

				explorerView.scrollRef.current.scrollBy({
					top: scrollBy,
					behavior: 'smooth'
				});
			}
		}
	});

	useWindowEventListener('mouseup', () => {
		if (resizing) {
			setTimeout(() => {
				setResizing(false);
				if (layout?.ref.current) {
					layout.ref.current.style.cursor = '';
				}
			});
		}
	});

	useMutationObserver(explorerView.scrollRef, () =>
		setListOffset(tableRef.current?.offsetTop ?? 0)
	);

	useLayoutEffect(() => setListOffset(tableRef.current?.offsetTop ?? 0), []);

	return (
		<div className="flex w-full flex-col" ref={tableRef}>
			{sized && (
				<ScrollSync>
					<>
						<ScrollSyncPane>
							<div
								className={clsx(
									'no-scrollbar table-header-group overflow-x-auto overscroll-x-none',
									isScrolled && 'top-bar-blur fixed z-20 !bg-app/90'
								)}
								style={{
									top: top,
									width: isScrolled ? tableWidth : undefined
								}}
							>
								<div className="flex">
									{table.getHeaderGroups().map((headerGroup) => (
										<div
											ref={tableHeaderRef}
											key={headerGroup.id}
											className="flex grow border-b border-app-line/50"
											onMouseDown={(e) => e.stopPropagation()}
										>
											{headerGroup.headers.map((header, i) => {
												const size = header.column.getSize();

												const isSorted =
													explorerStore.orderBy === header.id;

												const cellContent = flexRender(
													header.column.columnDef.header,
													header.getContext()
												);

												return (
													<div
														key={header.id}
														className="relative shrink-0 px-4 py-2 text-xs first:pl-24"
														style={{
															width:
																i === 0
																	? size + paddingX
																	: i ===
																	  headerGroup.headers.length - 1
																	? size +
																	  paddingX +
																	  scrollBarWidth
																	: size
														}}
														onClick={() => {
															if (resizing) return;

															if (header.column.getCanSort()) {
																if (isSorted) {
																	getExplorerStore().orderByDirection =
																		explorerStore.orderByDirection ===
																		'Asc'
																			? 'Desc'
																			: 'Asc';
																} else {
																	getExplorerStore().orderBy =
																		header.id as FilePathSearchOrderingKeys;
																}
															}
														}}
													>
														{header.isPlaceholder ? null : (
															<div
																className={clsx(
																	'flex items-center justify-between gap-3',
																	isSorted
																		? 'text-ink'
																		: 'text-ink-dull'
																)}
															>
																{typeof cellContent ===
																	'string' && (
																	<HeaderColumnName
																		name={cellContent}
																	/>
																)}

																{isSorted ? (
																	explorerStore.orderByDirection ===
																	'Asc' ? (
																		<CaretUp className="shrink-0 text-ink-faint" />
																	) : (
																		<CaretDown className="shrink-0 text-ink-faint" />
																	)
																) : null}

																<div
																	onClick={(e) =>
																		e.stopPropagation()
																	}
																	onMouseDown={(e) => {
																		header.getResizeHandler()(
																			e
																		);

																		setResizing(true);
																		setLocked(false);

																		if (layout?.ref.current) {
																			layout.ref.current.style.cursor =
																				'col-resize';
																		}
																	}}
																	onTouchStart={header.getResizeHandler()}
																	className="absolute right-0 h-[70%] w-2 cursor-col-resize border-r border-app-line/50"
																/>
															</div>
														)}
													</div>
												);
											})}
										</div>
									))}
								</div>
							</div>
						</ScrollSyncPane>

						<ScrollSyncPane>
							<div className="no-scrollbar overflow-x-auto overscroll-x-none">
								<div
									ref={tableBodyRef}
									className="relative"
									style={{
										height: `${rowVirtualizer.getTotalSize()}px`,
										width: headerWidth
									}}
								>
									{virtualRows.map((virtualRow) => {
										if (!explorerView.items) {
											return (
												<div
													key={virtualRow.index}
													className="absolute left-0 top-0 flex w-full py-px"
													style={{
														height: `${virtualRow.size}px`,
														transform: `translateY(${
															virtualRow.start -
															rowVirtualizer.options.scrollMargin
														}px)`,
														paddingLeft: `${paddingX}px`,
														paddingRight: `${paddingX}px`
													}}
												>
													<div className="relative flex h-full w-full animate-pulse rounded-md bg-app-box" />
												</div>
											);
										}

										const row = rows[virtualRow.index];
										if (!row) return null;

										const selected = isSelected(row.original.item.id);

										const previousRow = rows[virtualRow.index - 1];
										const selectedPrior =
											previousRow && isSelected(previousRow.original.item.id);

										const nextRow = rows[virtualRow.index + 1];
										const selectedNext =
											nextRow && isSelected(nextRow.original.item.id);

										const cut = isCut(row.original.item.id);

										return (
											<div
												key={row.id}
												className="absolute left-0 top-0 flex w-full"
												style={{
													height: `${virtualRow.size}px`,
													transform: `translateY(${
														virtualRow.start -
														rowVirtualizer.options.scrollMargin
													}px)`,
													paddingLeft: `${paddingX}px`,
													paddingRight: `${paddingX}px`
												}}
											>
												<div
													onMouseDown={(e) => {
														e.stopPropagation();
														handleRowClick(e, row);
													}}
													onContextMenu={() => handleRowContextMenu(row)}
													className={clsx(
														'relative flex h-full w-full rounded-md border',
														virtualRow.index % 2 === 0 &&
															'bg-app-darkBox',
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
												>
													{selectedPrior && (
														<div className="absolute inset-x-3 top-0 h-px bg-accent/10" />
													)}

													<ListViewItem
														row={row}
														paddingX={paddingX}
														columnSizing={columnSizing}
														selected={selected}
														cut={cut}
													/>
												</div>
											</div>
										);
									})}
								</div>
							</div>
						</ScrollSyncPane>
					</>
				</ScrollSync>
			)}
		</div>
	);
};
