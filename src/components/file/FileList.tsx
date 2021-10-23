import { DocumentIcon, DotsVerticalIcon, FilmIcon, FolderIcon } from '@heroicons/react/solid';
import clsx from 'clsx';
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { IFile } from '../../types';
import byteSize from 'pretty-bytes';
import { useKey } from 'rooks';
import { invoke } from '@tauri-apps/api';
import {
  useCurrentDir,
  useExplorerStore,
  useFile,
  useSelectedFile,
  useSelectedFileIndex
} from '../../store/explorer';
import { DirectoryResponse } from '../../screens/Explorer';

interface Column {
  column: string;
  key: string;
  width: number;
}

// Function ensure no types are loss, but guarantees that they are Column[]
function ensureIsColumns<T extends Column[]>(data: T) {
  return data;
}

const columns = ensureIsColumns([
  { column: 'Name', key: 'name', width: 280 } as const,
  { column: 'Size', key: 'size_in_bytes', width: 120 } as const,
  { column: 'Type', key: 'extension', width: 100 } as const
  // { column: 'Checksum', key: 'meta_checksum', width: 120 } as const
  // { column: 'Tags', key: 'tags', width: 120 } as const
]);

type ColumnKey = typeof columns[number]['key'];

export const FileList: React.FC<{}> = (props) => {
  const scrollContainer = useRef<null | HTMLDivElement>(null);
  const [rowHeight, setRowHeight] = useState(0);
  // const [selectedRow, setSelectedRow] = useState(0);
  const currentDir = useCurrentDir();
  console.log({ currentDir });

  if (!currentDir) return <></>;

  const explorer = useExplorerStore.getState();

  useKey('ArrowUp', (e) => {
    e.preventDefault();
    if (explorer.selectedFile) explorer.selectFile(currentDir.id, explorer.selectedFile, 'above');
  });
  useKey('ArrowDown', (e) => {
    e.preventDefault();
    if (explorer.selectedFile) explorer.selectFile(currentDir.id, explorer.selectedFile, 'below');
  });

  function isRowOutOfView(rowHeight: number, rowIndex: number) {
    const scrollTop = scrollContainer.current?.scrollTop || 0;
  }

  function handleScroll() {}

  return (
    <div
      ref={scrollContainer}
      onScroll={handleScroll}
      className="table-container w-full h-full overflow-scroll bg-white dark:bg-gray-900 p-3 cursor-default"
    >
      <div className="table-head">
        <div className="table-head-row flex flex-row p-2">
          {columns.map((col) => (
            <div
              key={col.key}
              className="table-head-cell flex flex-row items-center relative group px-4"
              style={{ width: col.width }}
            >
              <DotsVerticalIcon className="hidden absolute group-hover:block drag-handle w-5 h-5 opacity-10 -ml-5 cursor-move" />
              <span className="text-sm text-gray-500 font-medium">{col.column}</span>
            </div>
          ))}
        </div>
      </div>
      <div className="table-body pb-10">
        {currentDir?.children?.map((row, index) => (
          <RenderRow key={row.id} row={row} rowIndex={index} dirId={currentDir.id} />
        ))}
      </div>
    </div>
  );
};

const RenderRow: React.FC<{ row: IFile; rowIndex: number; dirId: number }> = ({
  row,
  rowIndex,
  dirId
}) => {
  const selectedFileIndex = useSelectedFileIndex(dirId);

  const isActive = selectedFileIndex === rowIndex;
  const isAlternate = rowIndex % 2 == 0;

  function selectFile() {
    if (selectedFileIndex == rowIndex) useExplorerStore.getState().clearSelectedFiles();
    else useExplorerStore.getState().selectFile(dirId, row.id);
  }

  return useMemo(
    () => (
      <div
        onClick={selectFile}
        onDoubleClick={() => {
          if (row.is_dir) {
            invoke<DirectoryResponse>('get_files', { path: row.uri }).then((res) => {
              useExplorerStore.getState().ingestDir(res.directory, res.contents);
            });
          }
        }}
        className={clsx('table-body-row flex flex-row rounded-lg border-2 border-[#00000000]', {
          'bg-[#00000006] dark:bg-[#00000030]': isAlternate,
          'border-primary-500': isActive
        })}
      >
        {columns.map((col) => (
          <div
            key={col.key}
            className="table-body-cell px-4 py-2 flex items-center pr-2"
            style={{ width: col.width }}
          >
            <RenderCell fileId={row.id} dirId={dirId} colKey={col?.key} />
          </div>
        ))}
      </div>
    ),
    [isActive]
  );
};

const RenderCell: React.FC<{ colKey?: ColumnKey; dirId?: number; fileId?: number }> = ({
  colKey,
  fileId,
  dirId
}) => {
  if (!fileId || !colKey || !dirId) return <></>;
  const row = useFile(fileId);
  if (!row) return <></>;
  const value = row[colKey];
  if (!value) return <></>;

  switch (colKey) {
    case 'name':
      return (
        <div className="flex flex-row items-center overflow-hidden">
          <div className="w-6 h-6 mr-2">
            {!!row?.icon_b64 && (
              <img src={'data:image/png;base64, ' + row.icon_b64} className="w-6 h-6 mr-2" />
            )}
          </div>
          {/* {colKey == 'name' &&
            (() => {
              switch (row.extension.toLowerCase()) {
                case 'mov' || 'mp4':
                  return <FilmIcon className="w-5 h-5 mr-3 flex-shrink-0 text-gray-300" />;

                default:
                  if (row.is_dir)
                    return <FolderIcon className="w-5 h-5 mr-3 flex-shrink-0 text-gray-300" />;
                  return <DocumentIcon className="w-5 h-5 mr-3 flex-shrink-0 text-gray-300" />;
              }
            })()} */}
          <span className="truncate text-xs">{row[colKey]}</span>
        </div>
      );
    case 'size_in_bytes':
      return <span className="text-xs text-left">{byteSize(Number(value || 0))}</span>;
    case 'extension':
      return <span className="text-xs text-left">{value.toLowerCase()}</span>;
    // case 'meta_checksum':
    //   return <span className="truncate">{value}</span>;
    // case 'tags':
    //   return renderCellWithIcon(MusicNoteIcon);

    default:
      return <></>;
  }
};
