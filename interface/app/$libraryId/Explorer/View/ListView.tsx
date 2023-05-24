import {
	ColumnDef,
	ColumnSizingState,
	Row,
	RowSelectionState,
	SortingState,
	flexRender,
	getCoreRowModel,
	getSortedRowModel,
	useReactTable
} from '@tanstack/react-table';
import { observeWindowOffset, useVirtualizer, useWindowVirtualizer } from '@tanstack/react-virtual';
import byteSize from 'byte-size';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { CaretDown, CaretUp } from 'phosphor-react';
import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useBoundingclientrect, useKey, useOnWindowResize } from 'rooks';
import useResizeObserver, { ObservedSize } from 'use-resize-observer';
import { ExplorerItem, FilePath, ObjectKind, isObject, isPath } from '@sd/client';
import { useDismissibleNoticeStore } from '~/hooks/useDismissibleNoticeStore';
import {
	getExplorerStore,
	getSelectedExplorerItems,
	useExplorerStore,
	useSelectedExplorerItems
} from '~/hooks/useExplorerStore';
import { useScrolled } from '~/hooks/useScrolled';
import { ViewItem } from '.';
import RenameTextBox from '../File/RenameTextBox';
import FileThumb from '../File/Thumb';
import { InfoPill } from '../Inspector';
import { useExplorerViewContext } from '../ViewContext';
import { getExplorerItemData, getItemFilePath } from '../util';

interface ListViewItemProps {
	row: Row<ExplorerItem>;
	index: number;
	selected: boolean;
	columnSizing: ColumnSizingState;
}

const ListViewItem = memo((props: ListViewItemProps) => {
	return (
		<ViewItem
			data={props.row.original}
			index={props.row.index}
			// className={clsx(
			// 	'flex w-full rounded-md border',
			// 	props.selected ? 'border-accent' : 'border-transparent',
			// 	props.index % 2 == 0 && 'bg-[#00000006] dark:bg-[#00000030]'
			// )}

			contextMenuClassName="w-full"
		>
			<div role="row" className={'flex h-full items-center'}>
				{props.row.getVisibleCells().map((cell) => {
					return (
						<div
							role="cell"
							key={cell.id}
							className={clsx(
								'table-cell truncate px-4 text-xs text-ink-dull',
								cell.column.columnDef.meta?.className
							)}
							style={{
								width: cell.column.getSize()
							}}
						>
							{flexRender(cell.column.columnDef.cell, cell.getContext())}
						</div>
					);
				})}
			</div>
		</ViewItem>
	);
});

export default () => {
	// const selectedExplorerItems = useSelectedExplorerItems();
	const explorerStore = useExplorerStore();
	const explorerView = useExplorerViewContext();

	const { isScrolled } = useScrolled(explorerView.scrollRef, 5);

	const [sized, setSized] = useState(false);
	const [sorting, setSorting] = useState<SortingState>([]);
	const [columnSizing, setColumnSizing] = useState<ColumnSizingState>({});
	const [rowSelection, setRowSelection] = useState<RowSelectionState>({});
	const [locked, setLocked] = useState(true);

	const [lastMouseSelectedId, setLastMouseSelectedId] = useState(0);
	const [lastKeySelectedId, setLastKeySelectedId] = useState<number>();

	const paddingX = 16;
	const scrollBarWidth = 8;

	const getObjectData = (data: ExplorerItem) => (isObject(data) ? data.item : data.item.object);
	const getFileName = (path: FilePath) => `${path.name}${path.extension && `.${path.extension}`}`;

	const columns = useMemo<ColumnDef<ExplorerItem>[]>(
		() => [
			{
				header: 'Name',
				minSize: 200,
				meta: { className: '!overflow-visible !text-ink' },
				accessorFn: (file) => {
					const filePathData = getItemFilePath(file);
					return filePathData && getFileName(filePathData);
				},
				cell: (cell) => {
					const file = cell.row.original;
					const filePathData = getItemFilePath(file);
					const selected = explorerStore.selectedRowIndex === cell.row.index;

					return (
						<div className="relative flex items-center">
							<div className="mr-[10px] flex h-6 w-12 shrink-0 items-center justify-center">
								<FileThumb data={file} size={35} />
							</div>
							{filePathData && (
								<RenameTextBox
									filePathData={filePathData}
									selected={selected}
									activeClassName="absolute z-50 top-0.5 left-[58px] max-w-[calc(100%-60px)]"
								/>
							)}
						</div>
					);
				}
			},
			{
				header: 'Type',
				accessorFn: (file) => {
					return isPath(file) && file.item.is_dir
						? 'Folder'
						: ObjectKind[getObjectData(file)?.kind || 0];
				},
				cell: (cell) => {
					const file = cell.row.original;
					return (
						<InfoPill className="bg-app-button/50">
							{isPath(file) && file.item.is_dir
								? 'Folder'
								: ObjectKind[getObjectData(file)?.kind || 0]}
						</InfoPill>
					);
				}
			},
			{
				header: 'Size',
				size: 100,
				accessorFn: (file) => byteSize(Number(getItemFilePath(file)?.size_in_bytes || 0))
			},
			{
				header: 'Date Created',
				accessorFn: (file) => dayjs(file.item.date_created).format('MMM Do YYYY'),
				sortingFn: (a, b, name) => {
					const aDate = a.original.item.date_created;
					const bDate = b.original.item.date_created;

					if (aDate === bDate) {
						const desc = sorting.find((s) => s.id === name)?.desc;

						const aPathData = getItemFilePath(a.original);
						const bPathData = getItemFilePath(b.original);

						const aName = aPathData ? getFileName(aPathData) : '';
						const bName = bPathData ? getFileName(bPathData) : '';

						return aName === bName
							? 0
							: aName > bName
							? desc
								? 1
								: -1
							: desc
							? -1
							: 1;
					}

					return aDate > bDate ? 1 : -1;
				}
			},
			{
				header: 'Content ID',
				size: 180,
				accessorFn: (file) => getExplorerItemData(file).casId
			}
		],
		[explorerStore.selectedRowIndex, explorerStore.isRenaming, sorting]
	);

	const table = useReactTable({
		data: explorerView.items || [],
		columns,
		defaultColumn: { minSize: 100 },
		state: { columnSizing, sorting, rowSelection },
		onColumnSizingChange: setColumnSizing,
		onSortingChange: setSorting,
		onRowSelectionChange: setRowSelection,
		columnResizeMode: 'onChange',
		getCoreRowModel: getCoreRowModel(),
		getSortedRowModel: getSortedRowModel()
	});

	const [listOffset, setListOffset] = useState(0);

	const tableLength = table.getTotalSize();
	const { rows } = table.getRowModel();

	const rowVirtualizer = useVirtualizer({
		count: explorerView.items ? rows.length : 100,
		getScrollElement: () => explorerView.scrollRef.current,
		estimateSize: () => 45,
		paddingStart: 12,
		paddingEnd: 12,
		scrollMargin: listOffset
	});

	const virtualRows = rowVirtualizer.getVirtualItems();

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

	function handleResize(size: ObservedSize) {
		if (!size.width) return;

		if (locked && Object.keys(columnSizing).length > 0) {
			table.setColumnSizing((sizing) => {
				const scrollWidth = size.width;
				const nameWidth = sizing.Name;
				return {
					...sizing,
					...(scrollWidth && nameWidth
						? {
								Name:
									nameWidth +
									scrollWidth -
									paddingX * 2 -
									scrollBarWidth -
									tableLength
						  }
						: {})
				};
			});
		} else {
			const scrollWidth = size.width;
			const tableWidth = tableLength;
			if (Math.abs(scrollWidth - tableWidth) < 10) {
				setLocked(true);
			}
		}
	}

	const tableRef = useRef<HTMLDivElement>(null);
	const tableBodyRef = useRef<HTMLDivElement>(null);

	const rect = useBoundingclientrect(tableRef);

	useResizeObserver({ onResize: handleResize, ref: tableRef });

	// TODO: Improve this
	useEffect(() => {
		setListOffset(tableRef.current?.offsetTop || 0);
	}, [rect]);

	// Measure initial column widths
	useEffect(() => {
		if (tableRef.current) {
			const columns = table.getAllColumns();
			const sizings = columns.reduce(
				(sizings, column) =>
					column.id === 'Name' ? sizings : { ...sizings, [column.id]: column.getSize() },
				{} as ColumnSizingState
			);
			const scrollWidth = tableRef.current.offsetWidth;
			const sizingsSum = Object.values(sizings).reduce((a, b) => a + b, 0);
			const nameWidth = scrollWidth - paddingX * 2 - scrollBarWidth - sizingsSum;
			table.setColumnSizing({ ...sizings, Name: nameWidth });
			setSized(true);
		}
	}, []);

	const [ranges, setRanges] = useState<[number, number][]>([[0, 0]]);

	const handleRowClick = (
		e: React.MouseEvent<HTMLDivElement, MouseEvent>,
		row: Row<ExplorerItem>
	) => {
		// console.log(row);
		const rowIndex = row.index;
		const itemId = row.original.item.id;
		// console.log(rows);
		if (e.shiftKey) {
			const range = ranges[ranges.length - 1];
			if (!range) return;

			const [rangeStart, rangeEnd] = range;

			const currentRowId = Number(row.id);
			const isCurrentHigher = currentRowId > rangeEnd;

			const ids = isCurrentHigher
				? Array.from(
						{ length: currentRowId - rangeEnd + (rangeEnd === 0 ? 1 : 0) },
						(_, i) => currentRowId - i
				  )
				: [];
			console.log(ids);

			const updated = new Set(explorerView.selectedItems);
			if (isCurrentHigher) {
				ids.forEach((id) => updated.add(Number(rows[id]?.original.item.id)));
			} else {
				ids.forEach((id) => updated.delete(Number(rows[id]?.original.item.id)));
			}

			explorerView.onSelectedChange?.(updated);
			setRanges([...ranges.slice(0, ranges.length - 1), [rangeStart, currentRowId]]);
		} else if (e.metaKey) {
			setLastMouseSelectedId(Number(row.id));

			const updated = new Set(explorerView.selectedItems);
			if (updated.has(itemId)) {
				updated.delete(itemId);
				setRanges(ranges.filter((range) => range[0] !== rowIndex));
			} else {
				console.log(ranges.slice(0, ranges.length));
				updated.add(itemId);
				setRanges([...ranges.slice(0, ranges.length - 1), [rowIndex, 0]]);
			}

			explorerView.onSelectedChange?.(updated);
		} else if (e.button === 0) {
			setLastMouseSelectedId(rowIndex);
			explorerView.onSelectedChange?.(new Set([itemId]));
			setRanges([[rowIndex, 0]]);
		}
	};

	// Select item with arrow up key
	useKey(
		'ArrowUp',
		(e) => {
			e.preventDefault();

			const selectedRows = table.getSelectedRowModel().flatRows;
			const lastSelectedRow = selectedRows[selectedRows.length - 1];

			if (lastSelectedRow) {
				const currentIndex = rows.findIndex((row) => row.index === lastSelectedRow.index);
				const nextRowId = Number(rows[currentIndex - 1]?.id);

				console.log(nextRowId);

				if (nextRowId !== undefined) {
					table.setRowSelection((current) => ({
						...(e.shiftKey ? current : {}),
						[nextRowId]: true
					}));
					setLastKeySelectedId(nextRowId);
				}
			}
		},
		{ when: !explorerStore.isRenaming }
	);

	// Select item with arrow down key
	useKey(
		'ArrowDown',
		(e) => {
			e.preventDefault();
			const selectedRows = table.getSelectedRowModel().flatRows;
			const lastSelectedRow = selectedRows[selectedRows.length - 1];

			if (
				lastSelectedRow &&
				explorerView.items &&
				lastSelectedRow.index !== explorerView.items.length - 1
			) {
				const currentIndex = rows.findIndex((row) => row.index === lastSelectedRow.index);
				const newIndex = rows[currentIndex + 1]?.id;

				if (newIndex !== undefined) {
					table.setRowSelection((current) => ({
						...(e.shiftKey ? current : {}),
						[newIndex]: true
					}));
					setLastKeySelectedId(Number(newIndex));
				}
			}
		},
		{ when: !explorerStore.isRenaming }
	);

	return (
		<div role="table" className="table w-full overflow-x-auto" ref={tableRef}>
			{sized && (
				<>
					<div
						onClick={(e) => e.stopPropagation()}
						className={clsx(
							'sticky  z-20 table-header-group',
							isScrolled && 'top-bar-blur !bg-app/90'
						)}
						style={{ top: explorerView.top || 0 }}
					>
						{table.getHeaderGroups().map((headerGroup) => (
							<div
								role="rowheader"
								key={headerGroup.id}
								className="flex border-b border-app-line/50"
							>
								{headerGroup.headers.map((header, i) => {
									const size = header.column.getSize();
									return (
										<div
											role="columnheader"
											key={header.id}
											className="relative truncate px-4 py-2 text-xs first:pl-24"
											style={{
												width:
													i === 0
														? size + paddingX
														: i === headerGroup.headers.length - 1
														? size - paddingX
														: size
											}}
											onClick={header.column.getToggleSortingHandler()}
										>
											{header.isPlaceholder ? null : (
												<div className={clsx('flex items-center')}>
													{flexRender(
														header.column.columnDef.header,
														header.getContext()
													)}
													<div className="flex-1" />

													{{
														asc: <CaretUp className="text-ink-faint" />,
														desc: (
															<CaretDown className="text-ink-faint" />
														)
													}[header.column.getIsSorted() as string] ??
														null}

													{(i !== headerGroup.headers.length - 1 ||
														(i === headerGroup.headers.length - 1 &&
															!locked)) && (
														<div
															onClick={(e) => e.stopPropagation()}
															onMouseDown={(e) => {
																setLocked(false);
																header.getResizeHandler()(e);
															}}
															onTouchStart={header.getResizeHandler()}
															className="absolute right-0 h-[70%] w-2 cursor-col-resize border-r border-app-line/50"
														/>
													)}
												</div>
											)}
										</div>
									);
								})}
							</div>
						))}
					</div>

					<div role="rowgroup" className="table-row-group" ref={tableBodyRef}>
						<div
							className="relative"
							style={{
								height: `${rowVirtualizer.getTotalSize()}px`
							}}
						>
							{virtualRows.map((virtualRow) => {
								if (!explorerView.items) {
									return (
										<div
											key={virtualRow.index}
											className="absolute left-0 top-0 flex w-full py-px pl-4 pr-3"
											style={{
												height: `${virtualRow.size}px`,
												transform: `translateY(${
													virtualRow.start -
													rowVirtualizer.options.scrollMargin
												}px)`
											}}
										>
											<div className="relative flex h-full w-full animate-pulse rounded-md bg-app-box" />
										</div>
									);
								}

								const row = rows[virtualRow.index];
								if (!row) return null;

								const selected = !!explorerView.selectedItems?.has(
									row.original.item.id
								);
								const selectedPrior = explorerView.selectedItems?.has(
									rows[virtualRow.index - 1]?.original.item.id!
								);
								const selectedNext = explorerView.selectedItems?.has(
									rows[virtualRow.index + 1]?.original.item.id!
								);

								return (
									<div
										key={row.id}
										className={clsx(
											'absolute left-0 top-0 flex w-full pl-4 pr-3',
											explorerStore.isRenaming && selected && 'z-10'
										)}
										style={{
											height: `${virtualRow.size}px`,
											transform: `translateY(${
												virtualRow.start -
												rowVirtualizer.options.scrollMargin
											}px)`
										}}
										onClick={(e) => handleRowClick(e, row)}
									>
										<div
											className={clsx(
												'relative flex h-full w-full rounded-md border',
												virtualRow.index % 2 === 0 &&
													'bg-[#00000006] dark:bg-[#00000030]',
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
												<div className="absolute left-3 right-3 top-0 h-px bg-accent/10" />
											)}

											<ListViewItem
												row={row}
												index={virtualRow.index}
												selected={selected}
												columnSizing={columnSizing}
											/>
										</div>
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
