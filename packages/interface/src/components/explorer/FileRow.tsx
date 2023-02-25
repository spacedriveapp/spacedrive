import byteSize from 'byte-size';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { HTMLAttributes } from 'react';
import { ExplorerItem, ObjectKind, isObject, isPath } from '@sd/client';
import { getExplorerStore } from '../../hooks/useExplorerStore';
import { ExplorerItemContextMenu } from './ExplorerContextMenu';
import { ColumnKey, columns } from './FileColumns';
import { FileThumb } from './FileThumb';
import { InfoPill } from './Inspector';
import { getExplorerItemData } from './util';

interface Props extends HTMLAttributes<HTMLDivElement> {
	data: ExplorerItem;
	index: number;
	selected: boolean;
}

function FileRow({ data, index, selected, ...props }: Props) {
	return (
		<ExplorerItemContextMenu className="w-full" data={data}>
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
						<RenderCell data={data} colKey={col.key} />
					</div>
				))}
			</div>
		</ExplorerItemContextMenu>
	);
}

const RenderCell: React.FC<{
	colKey: ColumnKey;
	data: ExplorerItem;
}> = ({ colKey, data }) => {
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
						{data.item.name}.{data.item.extension}
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
					{data.item.extension && (
						<span className="text-tiny text-ink-dull">{data.item.extension}</span>
					)}
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

export default FileRow;
