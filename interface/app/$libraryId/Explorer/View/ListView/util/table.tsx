import {
	getCoreRowModel,
	useReactTable,
	type ColumnDef,
	type ColumnSizingState,
	type VisibilityState
} from '@tanstack/react-table';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { useEffect, useMemo, useState } from 'react';
import { stringify } from 'uuid';
import {
	byteSize,
	getExplorerItemData,
	getItemFilePath,
	getItemObject,
	type ExplorerItem
} from '@sd/client';
import { isNonEmptyObject } from '~/util';

import { useExplorerContext } from '../../../Context';
import { FileThumb } from '../../../FilePath/Thumb';
import { InfoPill } from '../../../Inspector';
import { useQuickPreviewStore } from '../../../QuickPreview/store';
import { isCut, useExplorerStore } from '../../../store';
import { uniqueId } from '../../../util';
import RenamableItemText from '../../RenamableItemText';

export const useTable = () => {
	const explorer = useExplorerContext();
	const explorerStore = useExplorerStore();
	const quickPreviewStore = useQuickPreviewStore();

	const [columnSizing, setColumnSizing] = useState<ColumnSizingState>({});
	const [columnVisibility, setColumnVisibility] = useState<VisibilityState>({});

	const columns = useMemo<ColumnDef<ExplorerItem>[]>(
		() => [
			{
				id: 'name',
				header: 'Name',
				minSize: 200,
				maxSize: undefined,
				accessorFn: (file) => getExplorerItemData(file).fullName,
				cell: (cell) => {
					const item = cell.row.original;

					const selected = explorer.selectedItems.has(item);
					const cut = isCut(item, explorerStore.cutCopyState);

					return (
						<div className="relative flex items-center">
							<FileThumb
								data={item}
								size={35}
								blackBars
								className={clsx('mr-2.5', cut && 'opacity-60')}
							/>

							<RenamableItemText
								allowHighlight={false}
								item={item}
								selected={selected}
								disabled={
									!selected ||
									explorer.selectedItems.size > 1 ||
									quickPreviewStore.open
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
				accessorFn: (file) => getExplorerItemData(file).kind,
				cell: (cell) => (
					<InfoPill className="bg-app-button/50">
						{getExplorerItemData(cell.row.original).kind}
					</InfoPill>
				)
			},
			{
				id: 'sizeInBytes',
				header: 'Size',
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
				accessorFn: (file) => {
					const item = getItemFilePath(file);
					return dayjs(
						(item && 'date_indexed' in item && item.date_indexed) || null
					).format('MMM Do YYYY');
				}
			},
			{
				id: 'dateAccessed',
				header: 'Date Accessed',
				accessorFn: (file) =>
					getItemObject(file)?.date_accessed &&
					dayjs(getItemObject(file)?.date_accessed).format('MMM Do YYYY')
			},
			{
				id: 'contentId',
				header: 'Content ID',
				enableSorting: false,
				accessorFn: (file) => getExplorerItemData(file).casId
			},
			{
				id: 'objectId',
				header: 'Object ID',
				enableSorting: false,
				accessorFn: (file) => {
					const value = getItemObject(file)?.pub_id;
					if (!value) return null;
					return stringify(value);
				}
			}
		],
		[explorer.selectedItems, explorerStore.cutCopyState, quickPreviewStore.open]
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
