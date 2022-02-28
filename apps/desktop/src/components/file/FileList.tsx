import { DotsVerticalIcon } from '@heroicons/react/solid';
import { invoke } from '@tauri-apps/api';
import { convertFileSrc } from '@tauri-apps/api/tauri';
import clsx from 'clsx';
import byteSize from 'pretty-bytes';
import React, { forwardRef, useEffect, useMemo, useRef } from 'react';
import { Virtuoso, VirtuosoHandle } from 'react-virtuoso';

import { useKey, useWindowSize } from 'rooks';
import { DirectoryResponse } from '../../screens/Explorer';
// import { List, ListRowRenderer } from 'react-virtualized';
import { useAppState } from '../../store/global';
import {
  useCurrentDir,
  useExplorerStore,
  useFile,
  useSelectedFileIndex
} from '../../store/explorer';
import { IFile } from '../../types';

interface IColumn {
  column: string;
  key: string;
  width: number;
}

const PADDING_SIZE = 130;

// Function ensure no types are loss, but guarantees that they are Column[]
function ensureIsColumns<T extends IColumn[]>(data: T) {
  return data;
}

const columns = ensureIsColumns([
  { column: 'Name', key: 'name', width: 280 } as const,
  { column: 'Size', key: 'size_in_bytes', width: 120 } as const,
  { column: 'Type', key: 'extension', width: 100 } as const
  // { column: 'Checksum', key: 'meta_integrity_hash', width: 120 } as const
  // { column: 'Tags', key: 'tags', width: 120 } as const
]);

type ColumnKey = typeof columns[number]['key'];

export const FileList: React.FC<{}> = (props) => {
  const tableContainer = useRef<null | HTMLDivElement>(null);
  const VList = useRef<null | VirtuosoHandle>(null);
  const currentDir = useCurrentDir();

  // useOnWindowResize((e) => {

  // })

  const size = useWindowSize();

  const explorer = useExplorerStore.getState();

  const seletedRowIndex = useSelectedFileIndex(currentDir?.id as number);
  useEffect(() => {
    // VList.current?.scrollIntoView()
    if (seletedRowIndex != null) VList.current?.scrollIntoView({ index: seletedRowIndex });
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

  const Row = (index: number) => {
    const row = currentDir?.children?.[index] as IFile;

    return <RenderRow key={index} row={row} rowIndex={index} dirId={currentDir?.id as number} />;
  };

  const Header = () => (
    <div>
      <h1 className="p-2 mt-10 ml-1 text-xl font-bold">{currentDir?.name}</h1>
      <div className="table-head">
        <div className="flex flex-row p-2 table-head-row">
          {columns.map((col) => (
            <div
              key={col.key}
              className="relative flex flex-row items-center pl-2 table-head-cell group"
              style={{ width: col.width }}
            >
              <DotsVerticalIcon className="absolute hidden w-5 h-5 -ml-5 cursor-move group-hover:block drag-handle opacity-10" />
              <span className="text-sm font-medium text-gray-500">{col.column}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );

  return useMemo(
    () => (
      <div
        ref={tableContainer}
        style={{ marginTop: -44 }}
        className="w-full h-full p-3 bg-white cursor-default table-container dark:bg-gray-900"
      >
        <Virtuoso
          data={currentDir?.children}
          ref={VList}
          // style={{ height: '400px' }}
          totalCount={currentDir?.children_count || 0}
          itemContent={Row}
          components={{ Header }}
          className="pb-10 outline-none table-body"
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
            className="flex items-center px-4 py-2 pr-2 table-body-cell"
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
                `${useAppState.getState().config.file_type_thumb_dir}/${
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
                  return <FilmIcon className="flex-shrink-0 w-5 h-5 mr-3 text-gray-300" />;

                default:
                  if (row.is_dir)
                    return <FolderIcon className="flex-shrink-0 w-5 h-5 mr-3 text-gray-300" />;
                  return <DocumentIcon className="flex-shrink-0 w-5 h-5 mr-3 text-gray-300" />;
              }
            })()} */}
          <span className="text-xs truncate">{row[colKey]}</span>
        </div>
      );
    case 'size_in_bytes':
      return <span className="text-xs text-left">{byteSize(Number(value || 0))}</span>;
    case 'extension':
      return <span className="text-xs text-left">{value.toLowerCase()}</span>;
    // case 'meta_integrity_hash':
    //   return <span className="truncate">{value}</span>;
    // case 'tags':
    //   return renderCellWithIcon(MusicNoteIcon);

    default:
      return <></>;
  }
};
