import {
	CellContext,
	getCoreRowModel,
	useReactTable,
	type ColumnDef,
	type ColumnSizingState,
	type VisibilityState
} from '@tanstack/react-table';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { memo, useEffect, useMemo, useState } from 'react';
import { stringify } from 'uuid';
import {
	byteSize,
	getExplorerItemData,
	getIndexedItemFilePath,
	getItemFilePath,
	getItemObject,
	useSelector,
	type ExplorerItem
} from '@sd/client';
import { isNonEmptyObject } from '~/util';

import { useExplorerContext } from '../../Context';
import { FileThumb } from '../../FilePath/Thumb';
import { InfoPill } from '../../Inspector';
import { CutCopyState, explorerStore, isCut } from '../../store';
import { uniqueId } from '../../util';
import { RenamableItemText } from '../RenamableItemText';

const NameCell = memo(({ item, selected }: { item: ExplorerItem; selected: boolean }) => {
	const cutCopyState = useSelector(explorerStore, (s) => s.cutCopyState);

	const cut = useMemo(() => isCut(item, cutCopyState as CutCopyState), [cutCopyState, item]);

	return (
		<div className="relative flex items-center">
			<FileThumb
				data={item}
				frame
				frameClassName="!border"
				blackBars
				size={35}
				className={clsx('mr-2.5', cut && 'opacity-60')}
			/>

			<RenamableItemText
				item={item}
				selected={selected}
				allowHighlight={false}
				style={{ maxHeight: 36 }}
				idleClassName="w-full !max-h-5"
			/>
		</div>
	);
});

type Cell = CellContext<ExplorerItem, unknown> & { selected?: boolean };

export const useTable = () => {
	const explorer = useExplorerContext();

	const [columnSizing, setColumnSizing] = useState<ColumnSizingState>({});
	const [columnVisibility, setColumnVisibility] = useState<VisibilityState>({});

	const columns = useMemo<ColumnDef<ExplorerItem>[]>(
		() => [
			{
				id: 'name',
				header: 'Name',
				minSize: 200,
				maxSize: undefined,
				cell: ({ row, selected }: Cell) => (
					<NameCell item={row.original} selected={!!selected} />
				)
			},
			{
				id: 'kind',
				header: 'Type',
				cell: ({ row }) => (
					<InfoPill className="bg-app-button/50">
						{getExplorerItemData(row.original).kind}
					</InfoPill>
				)
			},
			{
				id: 'sizeInBytes',
				header: 'Size',
				accessorFn: (item) => {
					const filePath = getItemFilePath(item);
					return !filePath ||
						!filePath.size_in_bytes_bytes ||
						(filePath.is_dir && item.type === 'NonIndexedPath')
						? '-'
						: byteSize(filePath.size_in_bytes_bytes);
				}
			},
			{
				id: 'dateCreated',
				header: 'Date Created',
				accessorFn: (item) => {
					if (item.type === 'SpacedropPeer') return;
					return dayjs(item.item.date_created).format('MMM Do YYYY');
				}
			},
			{
				id: 'dateModified',
				header: 'Date Modified',
				accessorFn: (item) => {
					const filePath = getItemFilePath(item);
					if (filePath) return dayjs(filePath.date_modified).format('MMM Do YYYY');
				}
			},
			{
				id: 'dateIndexed',
				header: 'Date Indexed',
				accessorFn: (item) => {
					const filePath = getIndexedItemFilePath(item);
					if (filePath) return dayjs(filePath.date_indexed).format('MMM Do YYYY');
				}
			},
			{
				id: 'dateAccessed',
				header: 'Date Accessed',
				accessorFn: (item) => {
					const object = getItemObject(item);
					if (!object || !object.date_accessed) return;
					return dayjs(object.date_accessed).format('MMM Do YYYY');
				}
			},
			{
				id: 'contentId',
				header: 'Content ID',
				accessorFn: (item) => getExplorerItemData(item).casId
			},
			{
				id: 'objectId',
				header: 'Object ID',
				accessorFn: (item) => {
					const object = getItemObject(item);
					if (object) return stringify(object.pub_id);
				}
			}
		],
		[]
	);

	const table = useReactTable({
		data: useMemo(() => explorer.items ?? [], [explorer.items]),
		columns,
		defaultColumn: { minSize: 100, maxSize: 250 },
		state: { columnSizing, columnVisibility },
		onColumnVisibilityChange: setColumnVisibility,
		onColumnSizingChange: setColumnSizing,
		columnResizeMode: 'onChange',
		getCoreRowModel: useMemo(() => getCoreRowModel(), []),
		getRowId: uniqueId
	});

	// Initialize column visibility from explorer settings
	useEffect(() => {
		if (isNonEmptyObject(columnVisibility)) return;
		table.setColumnVisibility(explorer.settingsStore.colVisibility);
	}, [columnVisibility, explorer.settingsStore.colVisibility, table]);

	// Update column visibility in explorer settings
	// We don't update directly because it takes too long to get the updated values
	useEffect(() => {
		if (!isNonEmptyObject(columnVisibility)) return;
		explorer.settingsStore.colVisibility =
			columnVisibility as typeof explorer.settingsStore.colVisibility;
	}, [columnVisibility, explorer]);

	// Initialize column sizes from explorer settings
	useEffect(() => {
		if (isNonEmptyObject(columnSizing)) return;
		table.setColumnSizing(explorer.settingsStore.colSizes);
	}, [columnSizing, explorer.settingsStore.colSizes, table]);

	// Update column sizing in explorer settings
	// We don't update directly because it takes too long to get the updated values
	useEffect(() => {
		if (!isNonEmptyObject(columnSizing)) return;
		explorer.settingsStore.colSizes = columnSizing as typeof explorer.settingsStore.colSizes;
	}, [columnSizing, explorer]);

	return { table };
};
