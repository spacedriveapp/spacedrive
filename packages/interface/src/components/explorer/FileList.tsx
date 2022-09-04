import { EllipsisHorizontalIcon } from '@heroicons/react/24/solid';
import { LocationContext, useBridgeQuery, useExplorerStore, useLibraryQuery } from '@sd/client';
import { ExplorerContext, ExplorerItem, FilePath } from '@sd/core';
import clsx from 'clsx';
import React, { useContext, useEffect, useMemo, useRef, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { Virtuoso, VirtuosoGrid, VirtuosoHandle } from 'react-virtuoso';
import { useKey, useWindowSize } from 'rooks';
import styled from 'styled-components';

import FileItem from './FileItem';
import FileThumb from './FileThumb';
import { isPath } from './utils';

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

// these styled components are out of place, but are here to follow the virtuoso docs. could probably be translated to tailwind somehow, since the `components` prop only accepts a styled div, not a react component.
const GridContainer = styled.div`
	display: flex;
	margin-top: 60px;
	margin-left: 10px;
	width: 100%;
	flex-wrap: wrap;
`;
const GridItemContainer = styled.div`
	display: flex;
	flex-wrap: wrap;
`;

interface Props {
	context: ExplorerContext;
	data: ExplorerItem[];
}

export const FileList: React.FC<Props> = (props) => {
	const size = useWindowSize();
	const tableContainer = useRef<null | HTMLDivElement>(null);
	const VList = useRef<null | VirtuosoHandle>(null);

	const { data: client } = useBridgeQuery(['getNode'], {
		refetchOnWindowFocus: false
	});

	const { selectedRowIndex, set, layoutMode } = useExplorerStore();
	const [goingUp, setGoingUp] = useState(false);

	useEffect(() => {
		if (selectedRowIndex === 0 && goingUp) {
			VList.current?.scrollTo({ top: 0, behavior: 'smooth' });
		}
		if (selectedRowIndex !== -1 && typeof VList.current?.scrollIntoView === 'function') {
			VList.current?.scrollIntoView({
				index: goingUp ? selectedRowIndex - 1 : selectedRowIndex
			});
		}
	}, [goingUp, selectedRowIndex]);

	useKey('ArrowUp', (e) => {
		e.preventDefault();
		setGoingUp(true);
		if (selectedRowIndex !== -1 && selectedRowIndex !== 0)
			set({ selectedRowIndex: selectedRowIndex - 1 });
	});

	useKey('ArrowDown', (e) => {
		e.preventDefault();
		setGoingUp(false);
		if (selectedRowIndex !== -1 && selectedRowIndex !== (props.data.length ?? 1) - 1)
			set({ selectedRowIndex: selectedRowIndex + 1 });
	});

	const createRenderItem = (RenderItem: React.FC<RenderItemProps>) => {
		return (index: number) => {
			const row = props.data[index];
			if (!row) return null;
			return <RenderItem key={index} index={index} item={row} />;
		};
	};

	const Header = () => (
		<div>
			{props.context.name && (
				<h1 className="pt-20 pl-4 text-xl font-bold ">{props.context.name}</h1>
			)}
			<div className="table-head">
				<div className="flex flex-row p-2 table-head-row">
					{columns.map((col) => (
						<div
							key={col.key}
							className="relative flex flex-row items-center pl-2 table-head-cell group"
							style={{ width: col.width }}
						>
							<EllipsisHorizontalIcon className="absolute hidden w-5 h-5 -ml-5 cursor-move group-hover:block drag-handle opacity-10" />
							<span className="text-sm font-medium text-gray-500">{col.column}</span>
						</div>
					))}
				</div>
			</div>
		</div>
	);

	return (
		<div ref={tableContainer} style={{ marginTop: -44 }} className="w-full pl-2 cursor-default ">
			{layoutMode === 'grid' && (
				<VirtuosoGrid
					ref={VList}
					overscan={5000}
					components={{
						Item: GridItemContainer,
						List: GridContainer
					}}
					style={{ height: size.innerHeight ?? 600 }}
					totalCount={props.data.length || 0}
					itemContent={createRenderItem(RenderGridItem)}
					className="w-full overflow-x-hidden outline-none explorer-scroll"
				/>
			)}
			{layoutMode === 'list' && (
				<Virtuoso
					data={props.data} // this might be redundant, row data is retrieved by index in renderRow
					ref={VList}
					style={{ height: size.innerHeight ?? 600 }}
					totalCount={props.data.length || 0}
					itemContent={createRenderItem(RenderRow)}
					components={{
						Header,
						Footer: () => <div className="w-full " />
					}}
					increaseViewportBy={{ top: 400, bottom: 200 }}
					className="outline-none explorer-scroll"
				/>
			)}
		</div>
	);
};

interface RenderItemProps {
	item: ExplorerItem;
	index: number;
}

const RenderGridItem: React.FC<RenderItemProps> = ({ item, index }) => {
	const { selectedRowIndex, set } = useExplorerStore();
	const [_, setSearchParams] = useSearchParams();

	return (
		<FileItem
			onDoubleClick={() => {
				if (item.type === 'Path' && item.is_dir) {
					setSearchParams({ path: item.materialized_path });
				}
			}}
			index={index}
			data={item}
			selected={selectedRowIndex === index}
			onClick={() => {
				set({ selectedRowIndex: selectedRowIndex == index ? -1 : index });
			}}
			size={100}
		/>
	);
};

const RenderRow: React.FC<RenderItemProps> = ({ item, index }) => {
	const { selectedRowIndex, set } = useExplorerStore();
	const isActive = selectedRowIndex === index;
	const [_, setSearchParams] = useSearchParams();

	return useMemo(
		() => (
			<div
				onClick={() => set({ selectedRowIndex: selectedRowIndex == index ? -1 : index })}
				onDoubleClick={() => {
					if (isPath(item) && item.is_dir) {
						setSearchParams({ path: item.materialized_path });
					}
				}}
				className={clsx(
					'table-body-row mr-2 flex flex-row rounded-lg border-2',
					isActive ? 'border-primary-500' : 'border-transparent',
					index % 2 == 0 && 'bg-[#00000006] dark:bg-[#00000030]'
				)}
			>
				{columns.map((col) => (
					<div
						key={col.key}
						className="flex items-center px-4 py-2 pr-2 table-body-cell"
						style={{ width: col.width }}
					>
						<RenderCell data={item} colKey={col.key} />
					</div>
				))}
			</div>
		),
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[item.id, isActive]
	);
};

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
