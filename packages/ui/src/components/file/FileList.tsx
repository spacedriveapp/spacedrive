import { DotsVerticalIcon } from '@heroicons/react/solid';
import { useBridgeQuery } from '@sd/client';
import { FilePath } from '@sd/core';
import clsx from 'clsx';
import React, { useContext, useEffect, useMemo, useRef, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { Virtuoso, VirtuosoHandle } from 'react-virtuoso';
import { useKey, useWindowSize } from 'rooks';

import { AppPropsContext } from '../../AppPropsContext';
import { useExplorerState } from '../../hooks/useExplorerState';
import FileThumb from './FileThumb';

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
	// { column: 'Size', key: 'size_in_bytes', width: 120 } as const,
	{ column: 'Type', key: 'extension', width: 100 } as const
]);

type ColumnKey = typeof columns[number]['key'];

const LocationContext = React.createContext<{
	location_id: number;
	data_path: string;
}>({
	location_id: 1,
	data_path: ''
});

export const FileList: React.FC<{ location_id: number; path: string; limit: number }> = (props) => {
	const size = useWindowSize();
	const tableContainer = useRef<null | HTMLDivElement>(null);
	const VList = useRef<null | VirtuosoHandle>(null);

	const { data: client } = useBridgeQuery('NodeGetState', undefined, {
		refetchOnWindowFocus: false
	});

	const path = props.path;

	const { selectedRowIndex, setSelectedRowIndex, setLocationId } = useExplorerState();
	const [goingUp, setGoingUp] = useState(false);

	const { data: currentDir } = useBridgeQuery('LibGetExplorerDir', {
		location_id: props.location_id,
		path,
		limit: props.limit
	});

	useEffect(() => {
		if (selectedRowIndex === 0 && goingUp) {
			VList.current?.scrollTo({ top: 0, behavior: 'smooth' });
		}
		if (selectedRowIndex != -1) {
			VList.current?.scrollIntoView({
				index: goingUp ? selectedRowIndex - 1 : selectedRowIndex
			});
		}
	}, [selectedRowIndex]);

	useEffect(() => {
		setLocationId(props.location_id);
	}, [props.location_id]);

	useKey('ArrowUp', (e) => {
		e.preventDefault();
		setGoingUp(true);
		if (selectedRowIndex != -1 && selectedRowIndex !== 0) setSelectedRowIndex(selectedRowIndex - 1);
	});

	useKey('ArrowDown', (e) => {
		e.preventDefault();
		setGoingUp(false);
		if (selectedRowIndex != -1 && selectedRowIndex !== (currentDir?.contents.length ?? 1) - 1)
			setSelectedRowIndex(selectedRowIndex + 1);
	});

	const Row = (index: number) => {
		const row = currentDir?.contents?.[index];

		if (!row) return null;

		return <RenderRow key={index} row={row} rowIndex={index} dirId={currentDir?.directory.id} />;
	};

	const Header = () => (
		<div>
			<h1 className="pt-20 pl-4 text-xl font-bold ">{currentDir?.directory.name}</h1>
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
				className="w-full pl-2 bg-white cursor-default dark:bg-gray-650"
			>
				<LocationContext.Provider
					value={{ location_id: props.location_id, data_path: client?.data_path as string }}
				>
					<Virtuoso
						data={currentDir?.contents}
						ref={VList}
						style={{ height: size.innerHeight ?? 600 }}
						totalCount={currentDir?.contents.length || 0}
						itemContent={Row}
						components={{ Header, Footer: () => <div className="w-full " /> }}
						increaseViewportBy={{ top: 400, bottom: 200 }}
						className="outline-none explorer-scroll"
					/>
				</LocationContext.Provider>
			</div>
		),
		[props.location_id, size.innerWidth, currentDir?.directory.id, tableContainer.current]
	);
};

const RenderRow: React.FC<{
	row: FilePath;
	rowIndex: number;
	dirId: number;
}> = ({ row, rowIndex, dirId }) => {
	const { selectedRowIndex, setSelectedRowIndex } = useExplorerState();
	const isActive = selectedRowIndex === rowIndex;

	let [_, setSearchParams] = useSearchParams();

	function selectFileHandler() {
		if (selectedRowIndex == rowIndex) setSelectedRowIndex(-1);
		else setSelectedRowIndex(rowIndex);
	}

	return useMemo(
		() => (
			<div
				onClick={selectFileHandler}
				onDoubleClick={() => {
					if (row.is_dir) {
						setSearchParams({ path: row.materialized_path });
					}
				}}
				className={clsx(
					'table-body-row mr-2 flex flex-row rounded-lg border-2',
					isActive ? 'border-primary-500' : 'border-transparent',
					rowIndex % 2 == 0 && 'bg-[#00000006] dark:bg-[#00000030]'
				)}
			>
				{columns.map((col) => (
					<div
						key={col.key}
						className="flex items-center px-4 py-2 pr-2 table-body-cell"
						style={{ width: col.width }}
					>
						<RenderCell file={row} dirId={dirId} colKey={col?.key} />
					</div>
				))}
			</div>
		),
		[row.id, isActive]
	);
};

const RenderCell: React.FC<{
	colKey?: ColumnKey;
	dirId?: number;
	file?: FilePath;
}> = ({ colKey, file, dirId }) => {
	if (!file || !colKey || !dirId) return <></>;
	const row = file;
	if (!row) return <></>;
	const appProps = useContext(AppPropsContext);

	const value = row[colKey];
	if (!value) return <></>;

	const location = useContext(LocationContext);
	const { newThumbnails } = useExplorerState();

	const hasNewThumbnail = !!newThumbnails[row?.file?.cas_id ?? ''];

	switch (colKey) {
		case 'name':
			return (
				<div className="flex flex-row items-center overflow-hidden">
					<div className="flex items-center justify-center w-6 h-6 mr-3 shrink-0">
						<FileThumb
							hasThumbnailOverride={hasNewThumbnail}
							file={row}
							locationId={location.location_id}
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
		// case 'size_in_bytes':
		//   return <span className="text-xs text-left">{byteSize(Number(value || 0))}</span>;
		case 'extension':
			return <span className="text-xs text-left">{value}</span>;
		// case 'meta_integrity_hash':
		//   return <span className="truncate">{value}</span>;
		// case 'tags':
		//   return renderCellWithIcon(MusicNoteIcon);

		default:
			return <></>;
	}
};
