import { EllipsisHorizontalIcon } from '@heroicons/react/24/solid';
import { LocationContext, useBridgeQuery, useExplorerStore, useLibraryQuery } from '@sd/client';
import { ExplorerContext, ExplorerItem, FilePath } from '@sd/core';
import { useVirtualizer } from '@tanstack/react-virtual';
import clsx from 'clsx';
import React, { memo, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { Virtuoso, VirtuosoGrid, VirtuosoHandle } from 'react-virtuoso';
import { useKey, useWindowSize } from 'rooks';
import styled from 'styled-components';

import FileItem from './FileItem';
import FileThumb from './FileThumb';
import { isPath } from './utils';

interface Props extends React.HTMLAttributes<HTMLDivElement> {
	data: ExplorerItem;
	index: number;
	selected: boolean;
}

function FileRow({ data, index, selected, ...props }: Props) {
	return (
		<div
			{...props}
			className={clsx(
				'table-body-row mr-2 flex w-full flex-row rounded-lg border-2',
				selected ? 'border-primary-500' : 'border-transparent',
				index % 2 == 0 && 'bg-[#00000006] dark:bg-[#00000030]'
			)}
		>
			{columns.map((col) => (
				<div
					key={col.key}
					className="flex items-center px-4 py-2 pr-2 table-body-cell"
					style={{ width: col.width }}
				>
					<RenderCell data={data} colKey={col.key} />
				</div>
			))}
		</div>
	);
}

const RenderCell: React.FC<{
	colKey: ColumnKey;
	data: ExplorerItem;
}> = ({ colKey, data }) => {
	switch (colKey) {
		case 'name':
			return (
				<div className="flex flex-row items-center overflow-hidden">
					<div className="flex items-center justify-center w-6 h-6 mr-3 shrink-0">
						<FileThumb data={data} size={0} />
					</div>
					{/* {colKey == 'name' &&
            (() => {
              switch (row.extension.toLowerCase()) {
                case 'mov' || 'mp4':
                  return <FilmIcon className="flex-shrink-0 w-5 h-5 mr-3 text-gray-300" />;

                default:
                  if (row.is_dir)
                    return <FolderIcon className="flex-shrink-0 w-5 h-5 mr-3 text-gray-300" />;
                  return <DocumentIcon className="flex-shrink-0 w-5 h-5 mr-3 text-gray-300" />;
              }
            })()} */}
					<span className="text-xs truncate">{data[colKey]}</span>
				</div>
			);
		// case 'size_in_bytes':
		//   return <span className="text-xs text-left">{byteSize(Number(value || 0))}</span>;
		case 'extension':
			return <span className="text-xs text-left">{data[colKey]}</span>;
		// case 'meta_integrity_hash':
		//   return <span className="truncate">{value}</span>;
		// case 'tags':
		//   return renderCellWithIcon(MusicNoteIcon);

		default:
			return <></>;
	}
};

interface IColumn {
	column: string;
	key: string;
	width: number;
}

// Function ensure no types are lost, but guarantees that they are Column[]
function ensureIsColumns<T extends IColumn[]>(data: T) {
	return data;
}

const columns = ensureIsColumns([
	{ column: 'Name', key: 'name', width: 280 } as const,
	// { column: 'Size', key: 'size_in_bytes', width: 120 } as const,
	{ column: 'Type', key: 'extension', width: 100 } as const
]);

type ColumnKey = typeof columns[number]['key'];

export default FileRow;
