import { DocumentIcon, DotsVerticalIcon, FilmIcon, FolderIcon } from '@heroicons/react/solid';
import clsx from 'clsx';
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { IFile } from '../../types';
import byteSize from 'pretty-bytes';
import { useKey, useOnWindowResize, useWindowSize } from 'rooks';
import { invoke } from '@tauri-apps/api';
import {
  useCurrentDir,
  useExplorerStore,
  useFile,
  useSelectedFile,
  useSelectedFileIndex
} from '../../store/explorer';
import { DirectoryResponse } from '../../screens/Explorer';
import { List, ListRowRenderer } from 'react-virtualized';
import { useAppState } from '../../store/app';
import { convertFileSrc } from '@tauri-apps/api/tauri';

interface IColumn {
  column: string;
  key: string;
  width: number;
}

// Function ensure no types are loss, but guarantees that they are Column[]
function ensureIsColumns<T extends IColumn[]>(data: T) {
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
  const tableContainer = useRef<null | HTMLDivElement>(null);
  const VList = useRef<null | List>(null);
  const currentDir = useCurrentDir();

  // useOnWindowResize((e) => {

  // })

  const size = useWindowSize();

  const explorer = useExplorerStore.getState();

  const seletedRowIndex = useSelectedFileIndex(currentDir?.id as number);
  useEffect(() => {
    if (seletedRowIndex != null) VList.current?.scrollToRow(seletedRowIndex);
  }, [seletedRowIndex]);

  useKey('ArrowUp', (e) => {
    e.preventDefault();
    if (explorer.selectedFile) {
      explorer.selectFile(explorer.currentDir as number, explorer.selectedFile.id, 'above');
    }
  });
  useKey('ArrowDown', (e) => {
    e.preventDefault();
    if (explorer.selectedFile)
      explorer.selectFile(explorer.currentDir as number, explorer.selectedFile.id, 'below');
  });

  // function isRowOutOfView(rowHeight: number, rowIndex: number) {
  //   const scrollTop = scrollContainer.current?.scrollTop || 0;
  // }

  const rowRenderer: ListRowRenderer = ({
    index, // Index of row
    isScrolling, // The List is currently being scrolled
    isVisible, // This row is visible within the List (eg it is not an overscanned row)
    key, // Unique key within array of rendered rows
    parent, // Reference to the parent List (instance)
    style // Style object to be applied to row (to position it);
    // This must be passed through to the rendered row element.
  }) => {
    const row = currentDir?.children?.[index] as IFile;

    // If row content is complex, consider rendering a light-weight placeholder while scrolling.
    const content = (
      <RenderRow key={key} row={row} rowIndex={index} dirId={currentDir?.id as number} />
    );
    return (
      <div key={key} style={style}>
        {content}
      </div>
    );
  };

  const width = (tableContainer.current?.getBoundingClientRect().width || 0) - 30;
  const height = (tableContainer.current?.getBoundingClientRect().height || 0) - 140;

  return useMemo(
    () => (
      <div
        ref={tableContainer}
        className="table-container w-full h-full bg-white dark:bg-gray-900 p-3 cursor-default"
      >
        <h1 className="p-2 ml-3 font-bold text-xl">{currentDir?.name}</h1>
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
        <List
          ref={VList}
          width={width}
          height={height}
          rowHeight={40}
          rowCount={currentDir?.children_count || 0}
          rowRenderer={rowRenderer}
          className="table-body pb-10 outline-none"
        />
      </div>
    ),
    [size.innerWidth, currentDir?.id, tableContainer.current]
  );
};

const RenderRow: React.FC<{ row: IFile; rowIndex: number; dirId: number }> = ({
  row,
  rowIndex,
  dirId
}) => {
  const { selectFile, clearSelectedFiles, ingestDir } = useExplorerStore.getState();
  const selectedFileIndex = useSelectedFileIndex(dirId);

  const isActive = selectedFileIndex === rowIndex;
  // console.log('hello from row id', rowIndex);

  function selectFileHandler() {
    if (selectedFileIndex == rowIndex) clearSelectedFiles();
    else selectFile(dirId, row.id, undefined, rowIndex);
  }

  return useMemo(
    () => (
      <div
        onClick={selectFileHandler}
        onDoubleClick={() => {
          if (row.is_dir) {
            invoke<DirectoryResponse>('get_files', { path: row.uri }).then((res) => {
              ingestDir(res.directory, res.contents);
            });
          }
        }}
        className={clsx('table-body-row flex flex-row rounded-lg border-2 border-[#00000000]', {
          'bg-[#00000006] dark:bg-[#00000030]': rowIndex % 2 == 0,
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
    [row.id, isActive]
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

  // const icon = `${useAppState.getState().file_type_thumb_dir}/lol.png`;

  switch (colKey) {
    case 'name':
      return (
        <div className="flex flex-row items-center overflow-hidden">
          <div className="w-6 h-6 mr-2">
            <img
              src={convertFileSrc(
                `${useAppState.getState().file_type_thumb_dir}/${
                  row.is_dir ? 'folder' : row.extension
                }.png`
              )}
              className="w-6 h-6 mr-2"
            />
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
