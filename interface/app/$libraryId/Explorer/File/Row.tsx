import byteSize from 'byte-size';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { HTMLAttributes } from 'react';
import { ExplorerItem, ObjectKind, isObject, isPath } from '@sd/client';
import { InfoPill } from '../Inspector';
import { getExplorerItemData } from '../util';
import ContextMenu from './ContextMenu';
import FileThumb from './Thumb';

interface Props extends HTMLAttributes<HTMLDivElement> {
	data: ExplorerItem;
	index: number;
	selected: boolean;
}

export default ({ data, index, selected, ...props }: Props) => (
	<ContextMenu data={data}>
		<div
			{...props}
			className={clsx(
				'table-body-row mr-2 flex w-full flex-row rounded-lg border-2',
				selected ? 'border-accent' : 'border-transparent',
				index % 2 == 0 && 'bg-[#00000006] dark:bg-[#00000030]'
			)}
		>
			{columns.map((col) => (
				<div
					key={col.key}
					className="table-body-cell flex items-center px-4 py-2 pr-2"
					style={{ width: col.width }}
				>
					<Cell data={data} colKey={col.key} />
				</div>
			))}
		</div>
	</ContextMenu>
);

interface CellProps {
	colKey: (typeof columns)[number]['key'];
	data: ExplorerItem;
}

const Cell = ({ colKey, data }: CellProps) => {
	const objectData = data ? (isObject(data) ? data.item : data.item.object) : null;
	const { cas_id } = getExplorerItemData(data);

	switch (colKey) {
		case 'name':
			return (
				<div className="flex flex-row items-center overflow-hidden">
					<div className="mr-3 flex h-6 w-12 shrink-0 items-center justify-center">
						<FileThumb data={data} size={35} />
					</div>
					<span className="truncate text-xs">
						{data.item.name}
						{data.item.extension && `.${data.item.extension}`}
					</span>
				</div>
			);
		case 'size':
			return (
				<span className="text-ink-dull text-left text-xs font-medium">
					{byteSize(Number(objectData?.size_in_bytes || 0)).toString()}
				</span>
			);
		case 'date_created':
			return (
				<span className="text-ink-dull text-left text-xs font-medium">
					{dayjs(data.item?.date_created).format('MMM Do YYYY')}
				</span>
			);
		case 'cas_id':
			return <span className="text-ink-dull truncate text-left text-xs font-medium">{cas_id}</span>;
		case 'extension':
			return (
				<div className="flex flex-row items-center space-x-3">
					<InfoPill className="bg-app-button/50">
						{isPath(data) && data.item.is_dir ? 'Folder' : ObjectKind[objectData?.kind || 0]}
					</InfoPill>
				</div>
			);
		// case 'meta_integrity_hash':
		//   return <span className="truncate">{value}</span>;
		// case 'tags':
		//   return renderCellWithIcon(MusicNoteIcon);

		default:
			return <></>;
	}
};

interface Column {
	column: string;
	key: string;
	width: number;
}

export const columns = [
	{ column: 'Name', key: 'name', width: 280 },
	// { column: 'Size', key: 'size_in_bytes', width: 120 },
	{ column: 'Type', key: 'extension', width: 150 },
	{ column: 'Size', key: 'size', width: 100 },
	{ column: 'Date Created', key: 'date_created', width: 150 },
	{ column: 'Content ID', key: 'cas_id', width: 150 }
] as const satisfies Readonly<Column[]>;

export const ROW_HEADER_HEIGHT = 40;

export const RowHeader = () => (
	<div
		style={{ height: ROW_HEADER_HEIGHT }}
		className="mr-2 flex w-full flex-row rounded-lg border-2 border-transparent"
	>
		{columns.map((col) => (
			<div
				key={col.column}
				className="flex items-center px-4 py-2 pr-2"
				style={{ width: col.width, marginTop: -ROW_HEADER_HEIGHT * 2 }}
			>
				<span className="text-xs font-medium ">{col.column}</span>
			</div>
		))}
	</div>
);
