import {
	ColumnDef,
	ColumnSizingState,
	Row,
	flexRender,
	getCoreRowModel,
	useReactTable
} from '@tanstack/react-table';
import { useVirtualizer } from '@tanstack/react-virtual';
import byteSize from 'byte-size';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { CaretDown, CaretUp } from 'phosphor-react';
import { memo, useEffect, useMemo, useRef, useState } from 'react';
import { ScrollSync, ScrollSyncPane } from 'react-scroll-sync';
import { useBoundingclientrect, useKey } from 'rooks';
import useResizeObserver from 'use-resize-observer';
import { ExplorerItem, FilePath, ObjectKind, isObject, isPath } from '@sd/client';
import {
	FilePathSearchOrderingKeys,
	getExplorerStore,
	useExplorerStore
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
	columnSizing: ColumnSizingState;
	paddingX: number;
	selected: boolean;
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
							style={{
								width:
									cell.column.getSize() -
									(cells.length - 1 === i ? props.paddingX : 0)
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
	const explorerStore = useExplorerStore();
	const explorerView = useExplorerViewContext();

	const tableRef = useRef<HTMLDivElement>(null);
	const tableHeaderRef = useRef<HTMLDivElement>(null);
	const tableBodyRef = useRef<HTMLDivElement>(null);

	const [sized, setSized] = useState(false);
	const [locked, setLocked] = useState(true);
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

	const getObjectData = (data: ExplorerItem) => (isObject(data) ? data.item : data.item.object);
	const getFileName = (path: FilePath) => `${path.name}${path.extension && `.${path.extension}`}`;

	const columns = useMemo<ColumnDef<ExplorerItem>[]>(
		() => [
			{
				id: 'name',
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

					const selectedId = Array.isArray(explorerView.selected)
						? explorerView.selected[0]
						: explorerView.selected;

					const selected = selectedId === cell.row.original.item.id;

					return (
						<div className="relative flex items-center">
							<div className="mr-[10px] flex h-6 w-12 shrink-0 items-center justify-center">
								<FileThumb data={file} size={35} />
							</div>
							{filePathData && (
								<RenameTextBox
									filePathData={filePathData}
									disabled={
										!selected ||
										(Array.isArray(explorerView.selected) &&
											explorerView.selected.length > 1)
									}
									activeClassName="absolute z-50 top-0.5 left-[58px] max-w-[calc(100%-60px)]"
								/>
							)}
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
				id: 'sizeInBytes',
				header: 'Size',
				size: 100,
				accessorFn: (file) => byteSize(Number(getItemFilePath(file)?.size_in_bytes || 0))
			},
			{
				id: 'dateCreated',
				header: 'Date Created',
				accessorFn: (file) => dayjs(file.item.date_created).format('MMM Do YYYY')
			},
			{
				header: 'Content ID',
				enableSorting: false,
				size: 180,
				accessorFn: (file) => getExplorerItemData(file).casId
			}
		],
		[explorerView.selected]
	);

	const table = useReactTable({
		data: explorerView.items || [],
		columns,
		defaultColumn: { minSize: 100 },
		state: { columnSizing },
		onColumnSizingChange: setColumnSizing,
		columnResizeMode: 'onChange',
		getCoreRowModel: getCoreRowModel(),
		getRowId: (row) => String(row.item.id)
	});

	const tableLength = table.getTotalSize();
	const { rows } = table.getRowModel();

	const rowVirtualizer = useVirtualizer({
		count: explorerView.items ? rows.length : 100,
		getScrollElement: () => explorerView.scrollRef.current,
		estimateSize: () => rowHeight,
		paddingStart: paddingY + (isScrolled ? 35 : 0),
		paddingEnd: paddingY,
		scrollMargin: listOffset
	});

	const virtualRows = rowVirtualizer.getVirtualItems();

	const rect = useBoundingclientrect(tableRef);

	const selectedItems = useMemo(() => {
		return Array.isArray(explorerView.selected)
			? new Set(explorerView.selected)
			: explorerView.selected;
	}, [explorerView.selected]);

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
		} else {
			if (Math.abs(tableWidth - tableLength) < 10) {
				setLocked(true);
			}
		}
	}

	function handleRowClick(
		e: React.MouseEvent<HTMLDivElement, MouseEvent>,
		row: Row<ExplorerItem>
	) {
		if (!explorerView.onSelectedChange || e.button !== 0) return;

		const rowIndex = row.index;
		const itemId = row.original.item.id;

		if (e.shiftKey && Array.isArray(explorerView.selected)) {
			const range = ranges[ranges.length - 1];
			if (!range) return;

			const [rangeStartId, rangeEndId] = range;

			const rowsById = table.getCoreRowModel().rowsById;

			const rangeStartRow = table.getRow(String(rangeStartId));
			const rangeEndRow = table.getRow(String(rangeEndId));

			const lastDirection = rangeStartRow.index < rangeEndRow.index ? 'down' : 'up';
			const currentDirection = rangeStartRow.index < row.index ? 'down' : 'up';

			const currentRowIndex = row.index;

			const rangeEndItem = rowsById[rangeEndId];
			if (!rangeEndItem) return;

			const isCurrentHigher = currentRowIndex > rangeEndItem.index;

			const indexes = isCurrentHigher
				? Array.from(
						{
							length:
								currentRowIndex -
								rangeEndItem.index +
								(rangeEndItem.index === 0 ? 1 : 0)
						},
						(_, i) => rangeStartRow.index + i + 1
				  )
				: Array.from(
						{ length: rangeEndItem.index - currentRowIndex },
						(_, i) => rangeStartRow.index - (i + 1)
				  );

			const updated = new Set(explorerView.selected);
			if (isCurrentHigher) {
				indexes.forEach((i) => {
					updated.add(Number(rows[i]?.id));
				});
			} else {
				indexes.forEach((i) => updated.add(Number(rows[i]?.id)));
			}

			if (lastDirection !== currentDirection) {
				const sorted = Math.abs(rangeStartRow.index - rangeEndItem.index);

				const indexes = Array.from({ length: sorted }, (_, i) =>
					rangeStartRow.index < rangeEndItem.index
						? rangeStartRow.index + (i + 1)
						: rangeStartRow.index - (i + 1)
				);

				indexes.forEach(
					(i) => i !== rangeStartRow.index && updated.delete(Number(rows[i]?.id))
				);
			}
			explorerView.onSelectedChange?.([...updated]);
			setRanges([...ranges.slice(0, ranges.length - 1), [rangeStartId, itemId]]);
		} else if (e.metaKey && Array.isArray(explorerView.selected)) {
			const updated = new Set(explorerView.selected);
			if (updated.has(itemId)) {
				updated.delete(itemId);
				setRanges(ranges.filter((range) => range[0] !== rowIndex));
			} else {
				setRanges([...ranges.slice(0, ranges.length - 1), [itemId, itemId]]);
			}

			explorerView.onSelectedChange?.([...updated]);
		} else {
			explorerView.onSelectedChange(explorerView.multiSelect ? [itemId] : itemId);
			setRanges([[itemId, itemId]]);
		}
	}

	function handleRowContextMenu(row: Row<ExplorerItem>) {
		if (!explorerView.onSelectedChange || !explorerView.contextMenu) return;

		const itemId = row.original.item.id;

		if (
			!selectedItems ||
			(typeof selectedItems === 'object' && !selectedItems.has(itemId)) ||
			(typeof selectedItems === 'number' && selectedItems !== itemId)
		) {
			explorerView.onSelectedChange(typeof selectedItems === 'object' ? [itemId] : itemId);
			setRanges([[itemId, itemId]]);
		}
	}

	function isSelected(id: number) {
		return typeof selectedItems === 'object' ? !!selectedItems.has(id) : selectedItems === id;
	}

	useEffect(() => handleResize(), [tableWidth]);

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
					column.id === 'name' ? sizings : { ...sizings, [column.id]: column.getSize() },
				{} as ColumnSizingState
			);
			const scrollWidth = tableRef.current.offsetWidth;
			const sizingsSum = Object.values(sizings).reduce((a, b) => a + b, 0);
			const nameWidth = scrollWidth - paddingX * 2 - scrollBarWidth - sizingsSum;
			table.setColumnSizing({ ...sizings, name: nameWidth });
			setSized(true);
		}
	}, []);

	// initialize ranges
	useEffect(() => {
		if (ranges.length === 0 && explorerView.selected) {
			const id = Array.isArray(explorerView.selected)
				? explorerView.selected[explorerView.selected.length - 1]
				: explorerView.selected;

			if (id) setRanges([[id, id]]);
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

	useKey(
		['ArrowUp', 'ArrowDown'],
		(e) => {
			if (!explorerView.selectable) return;

			e.preventDefault();

			if (explorerView.onSelectedChange) {
				const lastSelectedItemId = Array.isArray(explorerView.selected)
					? explorerView.selected[explorerView.selected.length - 1]
					: explorerView.selected;

				if (lastSelectedItemId) {
					const lastSelectedRow = table.getRow(lastSelectedItemId.toString());

					if (lastSelectedRow) {
						const nextRow =
							rows[
								e.key === 'ArrowUp'
									? lastSelectedRow.index - 1
									: lastSelectedRow.index + 1
							];

						if (nextRow) {
							if (e.shiftKey && typeof selectedItems === 'object') {
								const newSet = new Set(selectedItems);

								if (
									selectedItems?.has(Number(nextRow.id)) &&
									selectedItems?.has(Number(lastSelectedRow.id))
								) {
									newSet.delete(Number(lastSelectedRow.id));
								} else {
									newSet.add(Number(nextRow.id));
								}

								explorerView.onSelectedChange([...newSet]);
								setRanges([
									...ranges.slice(0, ranges.length - 1),
									// FIXME: Eslint is right here.
									// eslint-disable-next-line @typescript-eslint/no-non-null-asserted-optional-chain
									[ranges[ranges.length - 1]?.[0]!, Number(nextRow.id)]
								]);
							} else {
								explorerView.onSelectedChange(
									explorerView.multiSelect
										? [Number(nextRow.id)]
										: Number(nextRow.id)
								);
								setRanges([[Number(nextRow.id), Number(nextRow.id)]]);
							}

							if (explorerView.scrollRef.current) {
								const tableBodyRect = tableBodyRef.current?.getBoundingClientRect();
								const scrollRect =
									explorerView.scrollRef.current.getBoundingClientRect();

								const paddingTop = parseInt(
									getComputedStyle(explorerView.scrollRef.current).paddingTop
								);

								const top =
									(explorerView.top
										? paddingTop + explorerView.top
										: paddingTop) +
									scrollRect.top +
									(isScrolled ? 35 : 0);

								const rowTop =
									nextRow.index * rowHeight +
									rowVirtualizer.options.paddingStart +
									(tableBodyRect?.top || 0) +
									scrollRect.top;

								const rowBottom = rowTop + rowHeight;

								if (rowTop < top) {
									const scrollBy =
										rowTop - top - (nextRow.index === 0 ? paddingY : 0);

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
						}
					}
				}
			}
		},
		{ when: !explorerView.isRenaming }
	);

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
										>
											{headerGroup.headers.map((header, i) => {
												const size = header.column.getSize();

												const isSorted =
													explorerStore.orderBy === header.id;

												return (
													<div
														key={header.id}
														className="relative shrink-0 truncate px-4 py-2 text-xs first:pl-24"
														style={{
															width:
																i === 0
																	? size + paddingX
																	: i ===
																	  headerGroup.headers.length - 1
																	? size - paddingX
																	: size
														}}
														onClick={() => {
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
																	'flex items-center'
																)}
															>
																{flexRender(
																	header.column.columnDef.header,
																	header.getContext()
																)}

																<div className="flex-1" />

																{isSorted ? (
																	explorerStore.orderByDirection ===
																	'Asc' ? (
																		<CaretUp className="shrink-0 text-ink-faint" />
																	) : (
																		<CaretDown className="shrink-0 text-ink-faint" />
																	)
																) : null}

																{(i !==
																	headerGroup.headers.length -
																		1 ||
																	(i ===
																		headerGroup.headers.length -
																			1 &&
																		!locked)) && (
																	<div
																		onClick={(e) =>
																			e.stopPropagation()
																		}
																		onMouseDown={(e) => {
																			setLocked(false);
																			header.getResizeHandler()(
																				e
																			);
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

										return (
											<div
												key={row.id}
												className={clsx(
													'absolute left-0 top-0 flex w-full',
													explorerView.isRenaming && selected && 'z-10'
												)}
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
