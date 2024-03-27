import {
	CellContext,
	functionalUpdate,
	getCoreRowModel,
	useReactTable,
	type ColumnDef
} from '@tanstack/react-table';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { memo, useMemo } from 'react';
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

import { useExplorerContext } from '../../Context';
import { FileThumb } from '../../FilePath/Thumb';
import { InfoPill } from '../../Inspector';
import { CutCopyState, explorerStore, isCut } from '../../store';
import { uniqueId } from '../../util';
import { RenamableItemText } from '../RenamableItemText';

export const LIST_VIEW_ICON_SIZES = {
	'0': 24,
	'1': 36,
	'2': 48
};

export const LIST_VIEW_TEXT_SIZES = {
	'0': 12,
	'1': 14,
	'2': 16
};

export const DEFAULT_LIST_VIEW_ICON_SIZE = '1' satisfies keyof typeof LIST_VIEW_ICON_SIZES;
export const DEFAULT_LIST_VIEW_TEXT_SIZE = '0' satisfies keyof typeof LIST_VIEW_TEXT_SIZES;

const NameCell = memo(({ item, selected }: { item: ExplorerItem; selected: boolean }) => {
	const cutCopyState = useSelector(explorerStore, (s) => s.cutCopyState);
	const cut = useMemo(() => isCut(item, cutCopyState as CutCopyState), [cutCopyState, item]);

	const explorer = useExplorerContext();
	const explorerSettings = explorer.useSettingsSnapshot();

	return (
		<div className="flex">
			<FileThumb
				data={item}
				frame
				frameClassName={clsx('!border', item.type === 'Label' && '!rounded-lg')}
				blackBars
				size={LIST_VIEW_ICON_SIZES[explorerSettings.listViewIconSize]}
				className={clsx('mr-2.5 transition-[height_width]', cut && 'opacity-60')}
			/>

			<div className="relative flex-1">
				<RenamableItemText
					item={item}
					selected={selected}
					allowHighlight={false}
					style={{ fontSize: LIST_VIEW_TEXT_SIZES[explorerSettings.listViewTextSize] }}
					className="absolute top-1/2 z-10 max-w-full -translate-y-1/2"
					idleClassName="!w-full"
					editLines={3}
				/>
			</div>
		</div>
	);
});

const KindCell = ({ kind }: { kind: string }) => {
	const explorer = useExplorerContext();
	const explorerSettings = explorer.useSettingsSnapshot();

	return (
		<InfoPill
			className="bg-app-button/50"
			style={{ fontSize: LIST_VIEW_TEXT_SIZES[explorerSettings.listViewTextSize] }}
		>
			{kind}
		</InfoPill>
	);
};

type Cell = CellContext<ExplorerItem, unknown> & { selected?: boolean };

export const useTable = () => {
	const explorer = useExplorerContext();
	const explorerSettings = explorer.useSettingsSnapshot();

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
				cell: ({ row }) => <KindCell kind={getExplorerItemData(row.original).kind} />
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
		state: {
			columnSizing: explorerSettings.colSizes,
			columnVisibility: explorerSettings.colVisibility
		},
		onColumnVisibilityChange: (updater) => {
			const visibility = functionalUpdate(updater, explorerSettings.colVisibility);
			explorer.settingsStore.colVisibility = {
				...explorerSettings.colVisibility,
				...visibility
			};
		},
		onColumnSizingChange: (updater) => {
			const sizing = functionalUpdate(updater, explorerSettings.colSizes);
			explorer.settingsStore.colSizes = {
				...explorerSettings.colSizes,
				...sizing
			};
		},
		columnResizeMode: 'onChange',
		getCoreRowModel: useMemo(() => getCoreRowModel(), []),
		getRowId: uniqueId
	});

	return { table };
};
