import { DocumentIcon, DotsVerticalIcon, FilmIcon, MusicNoteIcon } from '@heroicons/react/solid';
import clsx from 'clsx';
import React, { useMemo, useState } from 'react';
import { FileData } from '../../types';

interface Column {
  column: string;
  key: string;
  width: number;
}

interface FileListData {
  id: number;
  type?: string;
  tags?: Tag[];
  [key: string]: any;
}

interface Tag {
  name: string;
  color: string;
}

// Function ensure no types are loss, but guarantees that they are Column[]
function ensureIsColumns<T extends Column[]>(data: T) {
  return data;
}

const columns = ensureIsColumns([
  { column: 'Name', key: 'name', width: 280 } as const,
  { column: 'Size', key: 'size_in_bytes', width: 120 } as const,
  { column: 'Checksum', key: 'meta_checksum', width: 120 } as const
  // { column: 'Tags', key: 'tags', width: 120 } as const
]);

type ColumnKey = typeof columns[number]['key'];

// const data: FileListData[] = [
//   {
//     id: 1,
//     name: 'MyNameJeff.mp4',
//     type: 'video',
//     size: '5GB'
//     // tags: [{ name: 'Keepsafe', color: '#3076E6' }]
//   },
//   { id: 2, name: 'cow.ogg', type: 'audio', size: '345KB' },
//   { id: 3, name: 'calvin.fig', size: '34MB' },
//   { id: 4, name: 'calvin.fig' },
//   { id: 5, name: 'calvin.fig' },
//   { id: 6, name: 'calvin.fig' }
// ];

const RenderCell = ({ colKey, row }: { colKey?: ColumnKey; row?: FileData }) => {
  if (!row || !colKey || !row[colKey]) return <></>;

  const renderCellWithIcon = (Icon: any) => {
    return (
      <div className="flex flex-row items-center">
        {colKey == 'name' && <Icon className="w-5 h-5 mr-3 flex-shrink-0" />}
        <span className="truncate">{row[colKey]}</span>
      </div>
    );
  };

  switch (colKey) {
    case 'name':
      return renderCellWithIcon(FilmIcon);
    case 'size_in_bytes':
      return renderCellWithIcon(MusicNoteIcon);
    // case 'tags':
    //   return renderCellWithIcon(MusicNoteIcon);

    default:
      return <></>;
  }
};

export const FileList: React.FC<{ files: FileData[] }> = (props) => {
  const [selectedRow, setSelectedRow] = useState(0);
  return (
    <div className="table-container w-full h-full overflow-scroll bg-gray-900 rounded-md p-2 shadow-md">
      <div className="table-head">
        <div className="table-head-row flex flex-row p-2">
          {columns.map((col) => (
            <div
              key={col.key}
              className="table-head-cell flex flex-row items-center relative group px-4"
              style={{ width: col.width }}
            >
              <DotsVerticalIcon className="hidden absolute group-hover:block drag-handle w-5 h-5 opacity-10 -ml-5 cursor-move" />
              <span className="">{col.column}</span>
            </div>
          ))}
        </div>
      </div>
      <div className="table-body">
        {props.files?.map((row, index) => (
          <div
            key={row.id}
            onClick={() => setSelectedRow(row.id as number)}
            className={clsx('table-body-row flex flex-row rounded-lg border-2 border-[#00000000]', {
              'bg-[#00000030]': index % 2 == 0,
              'border-primary-500': selectedRow === row.id
            })}
          >
            {columns.map((col) => (
              <div key={col.key} className="table-body-cell px-4 py-2" style={{ width: col.width }}>
                {useMemo(
                  () => (
                    <RenderCell row={row} colKey={col?.key} />
                  ),
                  [row, col?.key]
                )}
              </div>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
};

// const columnKeyMap = columns.reduce((obj, column, index) => {
//   obj[column.key] = index;
//   return obj;
// }, {} as Record<string, number>);
