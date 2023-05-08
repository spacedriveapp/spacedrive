import {
	ColumnDef,
	ColumnSizingState,
	Row,
	SortingState,
	flexRender,
	getCoreRowModel,
	getSortedRowModel,
	useReactTable
} from '@tanstack/react-table';
import { useVirtualizer } from '@tanstack/react-virtual';
import byteSize from 'byte-size';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { CaretDown, CaretUp } from 'phosphor-react';
import { memo, useEffect, useMemo, useRef, useState } from 'react';
import { useKey, useOnWindowResize } from 'rooks';
import { ExplorerItem, FilePath, ObjectKind, isObject, isPath } from '@sd/client';
import { useDismissibleNoticeStore } from '~/hooks/useDismissibleNoticeStore';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { useScrolled } from '~/hooks/useScrolled';
import RenameTextBox from './File/RenameTextBox';
import Thumb from './File/Thumb';
import { InfoPill } from './Inspector';
import { ViewItem } from './View';
import { useExplorerViewContext } from './ViewContext';
import { getExplorerItemData, getItemFilePath } from './util';

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
			className={clsx(
				'flex w-full rounded-md border',
				props.selected ? 'border-accent' : 'border-transparent',
				props.index % 2 == 0 && 'bg-[#00000006] dark:bg-[#00000030]'
			)}
			contextMenuClassName="w-full"
		>
			<div role="row" className={'flex items-center'}>
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
	const explorerStore = useExplorerStore();
	const dismissibleNoticeStore = useDismissibleNoticeStore();
	const { data, scrollRef, onLoadMore, hasNextPage, isFetchingNextPage } =
		useExplorerViewContext();
	const { isScrolled } = useScrolled(scrollRef, 5);

	const [sized, setSized] = useState(false);
	const [sorting, setSorting] = useState<SortingState>([]);
	const [columnSizing, setColumnSizing] = useState<ColumnSizingState>({});
	const [locked, setLocked] = useState(true);

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
								<Thumb data={file} size={35} />
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
				accessorFn: (file) => getExplorerItemData(file).cas_id
			}
		],
		[explorerStore.selectedRowIndex, explorerStore.isRenaming, sorting]
	);

	const table = useReactTable({
		data,
		columns,
		defaultColumn: { minSize: 100 },
		state: { columnSizing, sorting },
		onColumnSizingChange: setColumnSizing,
		onSortingChange: setSorting,
		columnResizeMode: 'onChange',
		getCoreRowModel: getCoreRowModel(),
		getSortedRowModel: getSortedRowModel()
	});

	const tableLength = table.getTotalSize();
	const { rows } = table.getRowModel();

	const rowVirtualizer = useVirtualizer({
		count: rows.length,
		getScrollElement: () => scrollRef.current,
		estimateSize: () => 45,
		paddingStart: 12,
		paddingEnd: 12,
		overscan: !dismissibleNoticeStore.listView ? 5 : 1
	});

	const virtualRows = rowVirtualizer.getVirtualItems();

	useEffect(() => {
		const lastRow = virtualRows[virtualRows.length - 1];
		if (lastRow?.index === rows.length - 1 && hasNextPage && !isFetchingNextPage) {
			onLoadMore?.();
		}
	}, [hasNextPage, onLoadMore, isFetchingNextPage, virtualRows, rows.length]);

	function handleResize() {
		if (scrollRef.current) {
			if (locked && Object.keys(columnSizing).length > 0) {
				table.setColumnSizing((sizing) => {
					const scrollWidth = scrollRef.current?.offsetWidth;
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
				const scrollWidth = scrollRef.current.offsetWidth;
				const tableWidth = tableLength;
				if (Math.abs(scrollWidth - tableWidth) < 10) {
					setLocked(true);
				}
			}
		}
	}

	// Measure initial column widths
	useEffect(() => {
		if (scrollRef.current) {
			const columns = table.getAllColumns();
			const sizings = columns.reduce(
				(sizings, column) =>
					column.id === 'Name' ? sizings : { ...sizings, [column.id]: column.getSize() },
				{} as ColumnSizingState
			);
			const scrollWidth = scrollRef.current.offsetWidth;
			const sizingsSum = Object.values(sizings).reduce((a, b) => a + b, 0);
			const nameWidth = scrollWidth - paddingX * 2 - scrollBarWidth - sizingsSum;
			table.setColumnSizing({ ...sizings, Name: nameWidth });
			setSized(true);
		}
	}, []);

	// Resize view on window resize
	useOnWindowResize(handleResize);

	const lastSelectedIndex = useRef(explorerStore.selectedRowIndex);

	// Resize view on item selection/deselection
	useEffect(() => {
		const { selectedRowIndex } = explorerStore;

		if (
			explorerStore.showInspector &&
			typeof lastSelectedIndex.current !== typeof selectedRowIndex
		)
			handleResize();

		lastSelectedIndex.current = selectedRowIndex;
	}, [explorerStore.selectedRowIndex]);

	// Resize view on inspector toggle
	useEffect(() => {
		if (explorerStore.selectedRowIndex !== null) handleResize();
	}, [explorerStore.showInspector]);

	// Force recalculate range
	// https://github.com/TanStack/virtual/issues/485
	useMemo(() => {
		// @ts-ignore
		rowVirtualizer.calculateRange();
	}, [rows.length, rowVirtualizer]);

	// Select item with arrow up key
	useKey(
		'ArrowUp',
		(e) => {
			e.preventDefault();

			const { selectedRowIndex } = explorerStore;

			if (selectedRowIndex === null) return;

			if (selectedRowIndex > 0) {
				const currentIndex = rows.findIndex((row) => row.index === selectedRowIndex);
				const newIndex = rows[currentIndex - 1]?.index;

				if (newIndex !== undefined) getExplorerStore().selectedRowIndex = newIndex;
			}
		},
		{ when: !explorerStore.isRenaming }
	);

	// Select item with arrow down key
	useKey(
		'ArrowDown',
		(e) => {
			e.preventDefault();

			const { selectedRowIndex } = explorerStore;

			if (selectedRowIndex === null) return;

			if (selectedRowIndex !== data.length - 1) {
				const currentIndex = rows.findIndex((row) => row.index === selectedRowIndex);
				const newIndex = rows[currentIndex + 1]?.index;

				if (newIndex !== undefined) getExplorerStore().selectedRowIndex = newIndex;
			}
		},
		{ when: !explorerStore.isRenaming }
	);

	if (!sized) return null;
	return (
		<div role="table" className="table w-full overflow-x-auto">
			<div
				onClick={(e) => e.stopPropagation()}
				className={clsx(
					'sticky top-0 z-20 table-header-group',
					isScrolled && 'top-bar-blur !bg-app/90'
				)}
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
												desc: <CaretDown className="text-ink-faint" />
											}[header.column.getIsSorted() as string] ?? null}

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

			<div role="rowgroup" className="table-row-group">
				<div
					className="relative"
					style={{
						height: `${rowVirtualizer.getTotalSize()}px`
					}}
				>
					{virtualRows.map((virtualRow) => {
						const row = rows[virtualRow.index]!;
						const selected = explorerStore.selectedRowIndex === row.index;

						return (
							<div
								key={row.id}
								className={clsx(
									'absolute left-0 top-0 flex w-full pl-4 pr-3',
									explorerStore.isRenaming && selected && 'z-10'
								)}
								style={{
									height: `${virtualRow.size}px`,
									transform: `translateY(${virtualRow.start}px)`
								}}
							>
								<ListViewItem
									row={row}
									index={virtualRow.index}
									selected={selected}
									columnSizing={columnSizing}
								/>
							</div>
						);
					})}
				</div>
			</div>
		</div>
	);
};
