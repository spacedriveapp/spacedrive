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
	getExplorerItemData,
	getIndexedItemFilePath,
	getItemFilePath,
	getItemObject,
	humanizeSize,
	useExplorerLayoutStore,
	useSelector,
	type ExplorerItem
} from '@sd/client';
import { useLocale } from '~/hooks';

import { useExplorerContext } from '../../Context';
import { FileThumb } from '../../FilePath/Thumb';
import { InfoPill } from '../../Inspector';
import { CutCopyState, explorerStore, isCut } from '../../store';
import { translateKindName, uniqueId } from '../../util';
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
	const explorerLayout = useExplorerLayoutStore();

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
					className="absolute top-1/2 z-10 -translate-y-1/2"
					idleClassName={clsx(explorerLayout.showTags ? '!w-4/5' : '!w-full')}
					editLines={3}
				/>
				{explorerLayout.showTags && <Tags item={item} />}
			</div>
		</div>
	);
});

const Tags = ({ item }: { item: ExplorerItem }) => {
	const object = getItemObject(item);
	const filePath = getItemFilePath(item);
	const data = object || filePath;
	const tags = data && 'tags' in data ? data.tags : [];
	return (
		<div
			className="relative flex size-full flex-row items-center justify-end self-center"
			style={{
				marginLeft: tags.length * 4
			}}
		>
			{tags.map(({ tag }, i: number) => (
				<div
					key={tag.id}
					className="relative size-2.5 rounded-full border border-app"
					style={{
						backgroundColor: tag.color || 'transparent',
						right: i * 4
					}}
				/>
			))}
		</div>
	);
};

const KindCell = ({ kind }: { kind: string }) => {
	const explorer = useExplorerContext();
	const explorerSettings = explorer.useSettingsSnapshot();

	return (
		<InfoPill
			className="bg-app-button/50"
			style={{ fontSize: LIST_VIEW_TEXT_SIZES[explorerSettings.listViewTextSize] }}
		>
			{translateKindName(kind)}
		</InfoPill>
	);
};

type Cell = CellContext<ExplorerItem, unknown> & { selected?: boolean };

export const useTable = () => {
	const explorer = useExplorerContext();
	const explorerSettings = explorer.useSettingsSnapshot();

	const { t, dateFormat } = useLocale();

	const columns = useMemo<ColumnDef<ExplorerItem>[]>(
		() => [
			{
				id: 'name',
				header: t('name'),
				minSize: 200,
				maxSize: undefined,
				cell: ({ row, selected }: Cell) => (
					<NameCell item={row.original} selected={!!selected} />
				)
			},
			{
				id: 'kind',
				header: t('type'),
				cell: ({ row }) => <KindCell kind={getExplorerItemData(row.original).kind} />
			},
			{
				id: 'sizeInBytes',
				header: t('size'),
				accessorFn: (item) => {
					const filePath = getItemFilePath(item);
					return !filePath ||
						!filePath.size_in_bytes_bytes ||
						(filePath.is_dir && item.type === 'NonIndexedPath')
						? '-'
						: `${humanizeSize(filePath.size_in_bytes_bytes).value} ${t(`size_${humanizeSize(filePath.size_in_bytes_bytes).unit.toLowerCase()}`)}`;
				}
			},
			{
				id: 'dateCreated',
				header: t('date_created'),
				accessorFn: (item) => {
					if (item.type === 'SpacedropPeer') return;
					return dayjs(item.item.date_created).format(dateFormat);
				}
			},
			{
				id: 'dateModified',
				header: t('date_modified'),
				accessorFn: (item) => {
					const filePath = getItemFilePath(item);
					if (filePath) return dayjs(filePath.date_modified).format(dateFormat);
				}
			},
			{
				id: 'dateIndexed',
				header: t('date_indexed'),
				accessorFn: (item) => {
					const filePath = getIndexedItemFilePath(item);
					if (filePath) return dayjs(filePath.date_indexed).format(dateFormat);
				}
			},
			{
				id: 'dateAccessed',
				header: t('date_accessed'),
				accessorFn: (item) => {
					const object = getItemObject(item);
					if (!object || !object.date_accessed) return;
					return dayjs(object.date_accessed).format(dateFormat);
				}
			},
			{
				id: 'contentId',
				header: t('content_id'),
				accessorFn: (item) => getExplorerItemData(item).casId
			},
			{
				id: 'objectId',
				header: t('object_id'),
				accessorFn: (item) => {
					const object = getItemObject(item);
					if (object) return stringify(object.pub_id);
				}
			}
		],
		// eslint-disable-next-line react-hooks/exhaustive-deps
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
