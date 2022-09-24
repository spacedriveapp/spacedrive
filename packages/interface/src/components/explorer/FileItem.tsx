import { getExplorerStore, useExplorerStore } from '@sd/client';
import { ExplorerItem } from '@sd/core';
import clsx from 'clsx';
import { HTMLAttributes } from 'react';

import FileThumb from './FileThumb';
import { isObject } from './utils';

interface Props extends HTMLAttributes<HTMLDivElement> {
	data: ExplorerItem;
	selected: boolean;
	index: number;
}

function FileItem({ data, selected, index, ...rest }: Props) {
	// const store = useExplorerStore();

	// store.layoutMode;

	// props.index === store.selectedRowIndex

	return (
		<div
			onContextMenu={(e) => {
				const objectId = isObject(data) ? data.id : data.file?.id;
				if (objectId != undefined) {
					getExplorerStore().contextMenuObjectId = objectId;
					if (index != undefined) {
						getExplorerStore().selectedRowIndex = index;
					}
				}
			}}
			{...rest}
			draggable
			className={clsx('inline-block w-[100px] mb-3', rest.className)}
		>
			<div
				style={{ width: getExplorerStore().gridItemSize, height: getExplorerStore().gridItemSize }}
				className={clsx(
					'border-2 border-transparent rounded-lg text-center mb-1 active:translate-y-[1px]',
					{
						'bg-gray-50 dark:bg-gray-750': selected
					}
				)}
			>
				<div
					className={clsx(
						'flex items-center justify-center h-full  p-1 rounded border-transparent border-2 shrink-0'
					)}
				>
					<FileThumb
						className={clsx(
							'border-4  border-gray-250 rounded-sm shadow-md shadow-gray-750 object-cover max-w-full max-h-full w-auto overflow-hidden',
							isVideo(data.extension || '') && 'border-gray-950'
						)}
						data={data}
						size={getExplorerStore().gridItemSize}
					/>
				</div>
			</div>
			<div className="flex justify-center">
				<span
					className={clsx(
						'px-1.5 py-[1px] truncate text-center rounded-md text-xs font-medium text-gray-550 dark:text-gray-300 cursor-default ',
						{
							'bg-primary !text-white': selected
						}
					)}
				>
					{data?.name}
					{data?.extension && `.${data.extension}`}
				</span>
			</div>
		</div>
	);
}

export default FileItem;

function isVideo(extension: string) {
	return [
		'avi',
		'asf',
		'mpeg',
		'mts',
		'mpe',
		'vob',
		'qt',
		'mov',
		'asf',
		'asx',
		'mjpeg',
		'ts',
		'mxf',
		'm2ts',
		'f4v',
		'wm',
		'3gp',
		'm4v',
		'wmv',
		'mp4',
		'webm',
		'flv'
	].includes(extension);
}
